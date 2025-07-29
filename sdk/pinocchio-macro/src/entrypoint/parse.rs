#![allow(unused)]

use crate::classify;

use proc_macro2::TokenStream;
use proc_macro_error::abort;
use syn::ext::IdentExt as _;
use syn::parse::{Nothing, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse2, Attribute, Block, Error, Expr, Ident, Pat, PatType, Result, Token, Type};

pub fn parse(ts: TokenStream) -> Result<Ast> {
    parse2::<Ast>(ts)
}

/// Represent parsing result of the macro input token stream.
pub struct Ast {
    pub params: Parameters,
    pub cases: Vec<InstructionCase>,
}

impl Parse for Ast {
    fn parse(input: ParseStream) -> Result<Self> {
        let params = input.parse::<Parameters>()?;

        // `Parameters::parse` is expected stop just before a `;`, or it will produce a failure.
        let _ = input.parse::<Token![;]>()?;

        let mut cases = parse_cases(input)?;

        Ok(Self { params, cases })
    }
}

fn parse_cases(input: ParseStream) -> Result<Vec<InstructionCase>> {
    let mut res = vec![];
    let mut first_case = true;

    while !input.is_empty() {
        let value = InstructionCase::parse(first_case, input)?;
        res.push(value);

        first_case = false;
    }

    Ok(res)
}

/// Various parameters that control the macro execution.
///
/// Parser will only parse them one by one, allowing any to be absent.  `analyze` will perform any
/// further checking, including value checks, and checks for any required parameters.
#[derive(Default)]
pub struct Parameters {
    /// When generating code, we would need to reference types and functions from the `pinocchio`
    /// crate.  `proc_macro_crate` is used to guess the correct name for the `pinocchio` crate, but
    /// it does not always work.  This is the name of the crate to use when looking for pinocchio
    /// types and functions.
    pub pinocchio_crate: Option<(Ident, Ident)>,

    /// Type that will define how to parse the instruction byte(s) of the instruction data.
    pub instruction_type: Option<(Ident, Box<Type>)>,

    /// Name to give to the variable holding the program id.  Defaults to `program_id`.
    pub program_id_var_name: Option<(Ident, Ident)>,
}

impl Parse for Parameters {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut res = Parameters::default();

        loop {
            let argument_start = input.lookahead1();

            if argument_start.peek(Token![;]) {
                return Ok(res);
            } else if argument_start.peek(Ident) {
                // Processed below.
            } else {
                return Err(argument_start.error());
            }

            let ident = input.parse::<Ident>()?;

            if ident == "pinocchio_crate" {
                set_arg_if_new(
                    "pinocchio_crate",
                    &mut res.pinocchio_crate,
                    ident,
                    input,
                    Ident::parse,
                )?;
            } else if ident == "instruction_type" {
                set_arg_if_new(
                    "instruction_type",
                    &mut res.instruction_type,
                    ident,
                    input,
                    |input| Type::parse(input).map(Box::new),
                )?;
            } else if ident == "program_id" {
                set_arg_if_new(
                    "program_id",
                    &mut res.program_id_var_name,
                    ident,
                    input,
                    Ident::parse,
                )?;
            } else {
                return Err(Error::new(
                    ident.span(),
                    "Unexpected argument name.\n\
                     Known argument names: pinocchio_crate, instruction_type, program_id",
                ));
            }

            let argument_end = input.lookahead1();
            if argument_end.peek(Token![;]) {
                return Ok(res);
            } else if argument_end.peek(Token![,]) {
                let _ = input.parse::<Token![,]>()?;
            } else {
                return Err(argument_end.error());
            }
        }
    }
}

fn set_arg_if_new<T>(
    name: &'static str,
    target: &mut Option<(Ident, T)>,
    ident: Ident,
    input: ParseStream,
    parse_value: impl FnOnce(ParseStream) -> Result<T>,
) -> Result<()> {
    if let Some((prev, _)) = target {
        let mut err = Error::new(ident.span(), format!("Duplicate `{name}` argument."));
        let hint = Error::new(prev.span(), format!("Previous definition of `{name}`"));
        err.extend(hint);
        return Err(err);
    }

    let _ = input.parse::<Token![:]>()?;

    let value = parse_value(input)?;
    *target = Some((ident, value));

    Ok(())
}

pub struct InstructionCase {
    pub instruction_pat: Box<Pat>,
    // While we want to accept only a very limited set of types for the accounts, we will restrict
    // it later, in the `analyze` module.
    pub account_parsers: Vec<AccountParser>,
    pub data_parsers: Vec<DataParser>,
    pub body: InstructionBody,
}

impl InstructionCase {
    /// Almost `<InstructionCase as Parse>::parse()`, but we need to distinguish between parsing the
    /// first and subsequent instruction cases.
    ///
    /// `is_first` should be set to `true` when parsing the first instruction case.  It affects an
    /// error generated on unexpected `=>`, helping users that may thing that the arguments are
    /// separated by `;`.
    fn parse(is_first: bool, input: ParseStream) -> Result<InstructionCase> {
        let instruction_pat = Box::new(input.call(Pat::parse_multi_with_leading_vert)?);

        if is_first && matches!(instruction_pat.as_ref(), Pat::Ident(_)) && input.peek(Token![:]) {
            let colon = input.parse::<Token![:]>()?;
            return Err(Error::new(
                colon.spans[0],
                "expected `=>`.  Note: the macro arguments are separated with a `,`, not `;`.",
            ));
        }

        input.parse::<Token![=>]>()?;

        // closure open pipe
        input.parse::<Token![|]>()?;

        let mut account_parsers: Vec<AccountParser> = vec![];
        let mut seen_closure_args_end = false;
        loop {
            let mut attrs = vec![];

            // This block allows both for empty accounts section, as well as a trailing comma in the
            // accounts list.
            //
            // `Lookahead1` below should cover the error cases for the `;` and `|` in a nice way, so
            // it seems I do not need to worry about error handling here.
            if input.peek(Token![#]) {
                attrs = input.call(Attribute::parse_outer)?;
            } else if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
                break;
            } else if input.peek(Token![|]) {
                seen_closure_args_end = true;
                break;
            }

            let pat_type = input.parse::<PatType>()?;
            account_parsers.push(AccountParser { attrs, pat_type });

            let lookahead = input.lookahead1();
            if lookahead.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                continue;
            } else if lookahead.peek(Token![;]) {
                input.parse::<Token![;]>()?;
                break;
            } else if lookahead.peek(Token![|]) {
                seen_closure_args_end = true;
                break;
            } else {
                return Err(lookahead.error());
            }
        }

        let mut data_parsers: Vec<DataParser> = vec![];
        if !seen_closure_args_end {
            loop {
                let mut attrs = vec![];

                // This block allows for an empty data parsers section, even if `;` is used
                // explicitly.  And it also allows for a trailing comma after the last data parser
                // argument.
                //
                // `Lookahead1` below should cover the error cases for the `;` and `|` in a nice
                // way, so it seems I do not need to worry about error handling here.
                if input.peek(Token![#]) {
                    attrs = input.call(Attribute::parse_outer)?;
                } else if input.peek(Token![|]) {
                    break;
                }

                let pat_type = input.parse::<PatType>()?;
                data_parsers.push(DataParser { attrs, pat_type });

                let lookahead = input.lookahead1();
                if lookahead.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                    continue;
                } else if lookahead.peek(Token![|]) {
                    break;
                } else {
                    return Err(lookahead.error());
                }
            }
        }

        input.parse::<Token![|]>()?;

        let body: InstructionBody = input.parse()?;

        Ok(Self {
            instruction_pat,
            account_parsers,
            data_parsers,
            body,
        })
    }
}

pub struct AccountParser {
    pub attrs: Vec<Attribute>,
    pub pat_type: PatType,
}

pub struct DataParser {
    pub attrs: Vec<Attribute>,
    pub pat_type: PatType,
}

pub enum InstructionBody {
    Explicit(Box<Expr>),
    Implied,
}

impl Parse for InstructionBody {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![,]) {
            let _ = input.parse::<Token![,]>()?;
            return Ok(InstructionBody::Implied);
        } else if input.is_empty() {
            return Ok(InstructionBody::Implied);
        }

        let requires_comma;
        let body = {
            let body = Expr::parse_with_earlier_boundary_rule(input)?;
            requires_comma = classify::requires_comma_to_be_match_arm(&body);
            Box::new(body)
        };

        // Skip required and optional commas.
        let _: Option<Token![,]> = if requires_comma && !input.is_empty() {
            Some(input.parse()?)
        } else {
            input.parse()?
        };

        Ok(InstructionBody::Explicit(body))
    }
}

#[cfg(test)]
mod tests {
    use quote::{quote, ToTokens as _};
    use syn::parse_quote;

    use crate::test_utils::assert_macro_error;

    use super::*;

    #[track_caller]
    fn assert_params(
        actual: &Parameters,
        expected_pinocchio_crate: Option<&str>,
        expected_instruction_type: Option<&str>,
        expected_program_id: Option<&str>,
    ) {
        assert_eq!(
            actual
                .pinocchio_crate
                .as_ref()
                .map(|(_ident, value)| value.to_string()),
            expected_pinocchio_crate.map(str::to_owned),
        );
        assert_eq!(
            actual
                .instruction_type
                .as_ref()
                .map(|(_ident, value)| value.to_token_stream().to_string()),
            expected_instruction_type.map(str::to_owned),
        );
        assert_eq!(
            actual
                .program_id_var_name
                .as_ref()
                .map(|(_ident, value)| value.to_string()),
            expected_program_id.map(str::to_owned),
        );
    }

    #[test]
    fn single_mut_account() {
        let actual = parse(quote! {
            instruction_type: Instruction,
            program_id: test_program_id;

            Instruction::Setup => |metadata: Pin<&mut Account<T>>| {
                setup(metadata)
            }
        })
        .unwrap();

        assert_params(
            &actual.params,
            None,
            Some("Instruction"),
            Some("test_program_id"),
        );
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 0);
    }

    #[test]
    fn single_ref_account() {
        let actual = parse(quote! {
            instruction_type: Instruction,
            pinocchio_crate: renamed_pinocchio,
            program_id: me;

            Instruction::Setup => |metadata: &Account<T>| {
                setup(metadata)
            }
        })
        .unwrap();

        assert_params(
            &actual.params,
            Some("renamed_pinocchio"),
            Some("Instruction"),
            Some("me"),
        );
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 0);
    }

    #[test]
    fn single_mut_account_implied_fn() {
        let actual = parse(quote! {
            instruction_type: Instruction,
            program_id: test_program_id;

            Instruction::Setup => |metadata: Pin<&mut Account<T>>|,
        })
        .unwrap();

        assert_params(
            &actual.params,
            None,
            Some("Instruction"),
            Some("test_program_id"),
        );
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 0);
    }

    #[test]
    fn single_mut_account_implied_fn_no_last_comma() {
        let actual = parse(quote! {
            instruction_type: Instruction,
            program_id: test_program_id;

            Instruction::Setup => |metadata: Pin<&mut Account<T>>|
        })
        .unwrap();

        assert_params(
            &actual.params,
            None,
            Some("Instruction"),
            Some("test_program_id"),
        );
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 0);
    }

    #[test]
    fn single_account_with_attributes() {
        let actual = parse(quote! {
            ;

            Instruction::Setup => |
                #[signer]
                metadata: &Account<T>
            | {
                setup(metadata)
            }
        })
        .unwrap();

        assert_params(&actual.params, None, None, None);
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].account_parsers[0].attrs.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 0);
    }

    #[test]
    fn account_and_data_parser() {
        let actual = parse(quote! {
            program_id: program_address,
            instruction_type: ProgramInst;

            Instruction::NewUser => |metadata: Pin<&mut Account<T>>; name: &str| {
                new_user(metadata, name)
            }
        })
        .unwrap();

        assert_params(
            &actual.params,
            None,
            Some("ProgramInst"),
            Some("program_address"),
        );
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 1);
    }

    #[test]
    fn account_and_data_parser_with_attributes() {
        let actual = parse(quote! {
            instruction_type: Instruction,
            program_id: program_id,
            ;

            Instruction::NewUser => |
                metadata: Pin<&mut Account<T>>;
                #[max_len(50)]
                name: &str,
            | {
                new_user(metadata, name)
            }
        })
        .unwrap();

        assert_params(
            &actual.params,
            None,
            Some("Instruction"),
            Some("program_id"),
        );
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 1);
    }

    #[test]
    fn two_cases() {
        let actual = parse(quote! {
            instruction_type: Instruction,
            program_id: program_id,
            ;

            Instruction::NewUser => |metadata: Pin<&mut Account<Metadata>>; name: &str| {
                new_user(metadata, name)
            }

            Instruction::RenameUser => |
                metadata: Pin<&mut Account<Metadata>>,
                user: Pin<&mut Account<User>>,
                ;
                new_name: &str,
                log_entry: &str,
            | {
                rename_user(metadata, user, new_name)
            }
        })
        .unwrap();

        assert_params(
            &actual.params,
            None,
            Some("Instruction"),
            Some("program_id"),
        );
        assert_eq!(actual.cases.len(), 2);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 1);
        assert_eq!(actual.cases[1].account_parsers.len(), 2);
        assert_eq!(actual.cases[1].data_parsers.len(), 2);
    }
}
