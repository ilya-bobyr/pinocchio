#![allow(unused)]

use heck::ToSnakeCase as _;
use proc_macro2::{Span, TokenStream};
use quote::ToTokens as _;
use syn::{
    spanned::Spanned, Block, Error, Expr, Ident, Pat, PatIdent, PatPath, PatType, Path,
    PathSegment, Result, Type, TypeReference,
};

#[cfg(not(test))]
use proc_macro_crate::{crate_name, FoundCrate};

use crate::parse::{self, Ast};

pub struct Model {
    pub params: Parameters,
    pub cases: Vec<InstructionCase>,
}

pub struct Parameters {
    pub pinocchio_crate: Ident,
    pub instruction_type: Box<Type>,
    pub program_id_var_name: Ident,
}

pub struct InstructionCase {
    pub instruction_pat: Box<Pat>,
    pub account_parsers: Vec<AccountParser>,
    pub data_parsers: Vec<DataParser>,
    pub body: InstructionBody,
}

pub struct AccountParser {
    pub variable_pat: Box<Pat>,
    pub account_type: Box<Type>,
}

pub struct DataParser {
    pub variable_pat: Box<Pat>,
    pub data_type: Box<Type>,
}

pub enum InstructionBody {
    Explicit(Box<Expr>),
    Implied(Ident),
}

pub fn analyze(
    Ast {
        params:
            parse::Parameters {
                pinocchio_crate,
                instruction_type,
                program_id_var_name,
            },
        cases: ast_cases,
    }: Ast,
) -> Result<Model> {
    let params = Parameters {
        pinocchio_crate: find_pinocchio_crate(pinocchio_crate.map(|(_ident, val)| val))?,

        instruction_type: required_arg("instruction_type", instruction_type)?,
        program_id_var_name: required_arg("program_id", program_id_var_name)?,
    };

    let cases = ast_cases
        .into_iter()
        .map(analyze_instruction_case)
        .collect::<Result<_>>()?;

    Ok(Model { params, cases })
}

fn required_arg<T>(name: &'static str, provided: Option<(Ident, T)>) -> Result<T> {
    match provided {
        Some((ident, value)) => Ok(value),
        None => Err(Error::new(
            Span::call_site(),
            format!("`{name}` is a required argument"),
        )),
    }
}

fn analyze_instruction_case(
    parse::InstructionCase {
        instruction_pat,
        account_parsers: ast_accounts,
        data_parsers: ast_data,
        body,
    }: parse::InstructionCase,
) -> Result<InstructionCase> {
    if let Some(account_parser) = ast_accounts.get(64) {
        return Err(Error::new(
            account_parser.pat_type.span(),
            "No more than 64 accounts are currently supported per instruction.\n\
             This is the 65th account.",
        ));
    }

    let account_parsers = ast_accounts
        .into_iter()
        .map(analyze_account_parser)
        .collect::<Result<_>>()?;

    let data_parsers = ast_data
        .into_iter()
        .map(analyze_data_parser)
        .collect::<Result<_>>()?;

    let body = analyze_body(&instruction_pat, body)?;

    Ok(InstructionCase {
        instruction_pat,
        account_parsers,
        data_parsers,
        body,
    })
}

fn analyze_account_parser(
    parse::AccountParser {
        attrs,
        pat_type:
            PatType {
                attrs: pat_attrs,
                pat: variable_pat,
                colon_token: _,
                ty: account_type,
            },
    }: parse::AccountParser,
) -> Result<AccountParser> {
    if let Some(first_attr) = attrs.first() {
        return Err(Error::new(
            first_attr.span(),
            "Account attributes are not supported",
        ));
    }

    if let Some(first_attr) = pat_attrs.first() {
        return Err(Error::new(
            first_attr.span(),
            "Account type attributes are not supported",
        ));
    }

    Ok(AccountParser {
        variable_pat,
        account_type,
    })
}

fn analyze_data_parser(
    parse::DataParser {
        attrs,
        pat_type:
            PatType {
                attrs: pat_attrs,
                pat: variable_pat,
                colon_token: _,
                ty: data_type,
            },
    }: parse::DataParser,
) -> Result<DataParser> {
    if let Some(first_attr) = attrs.first() {
        return Err(Error::new(
            first_attr.span(),
            "Data parser attributes are not supported",
        ));
    }

    if let Some(first_attr) = pat_attrs.first() {
        return Err(Error::new(
            first_attr.span(),
            "Data parser type attributes are not supported",
        ));
    }

    Ok(DataParser {
        variable_pat,
        data_type,
    })
}

fn analyze_body(
    instruction_pat: &Pat,
    ast_body: parse::InstructionBody,
) -> Result<InstructionBody> {
    match ast_body {
        parse::InstructionBody::Explicit(body) => return Ok(InstructionBody::Explicit(body)),
        parse::InstructionBody::Implied => (),
    }

    let instruction_pattern_path_segments = match instruction_pat {
        Pat::Path(PatPath {
            attrs: _,
            qself: _,
            path: Path {
                leading_colon: _,
                segments,
            },
        }) => segments,
        _ => {
            return Err(Error::new(
                instruction_pat.span(),
                "An implied instruction function call is only supported when the \
                 instruction match expression is a path.  For example, `Instruction::MintToken`.",
            ));
        }
    };

    let Some(PathSegment {
        ident: last_path_segment,
        arguments: _,
    }) = instruction_pattern_path_segments.last()
    else {
        return Err(Error::new(
            instruction_pat.span(),
            "An implied instruction function call is only supported when the instruction match \
             expression is a non-empty path.  For example, `Instruction::MintToken`.  The last \
             path component must to be spelled explicitly.",
        ));
    };

    let invoke_fn = Ident::new(
        &last_path_segment.to_string().to_snake_case(),
        last_path_segment.span(),
    );

    Ok(InstructionBody::Implied(invoke_fn))
}

#[cfg(not(test))]
/// Looks for the name of the Pinocchio crate in the context of the macro invocation.
/// It can be explicitly overwritten by the user, provided by `provided_name`.
/// If not specified, `proc_macro_crate` is used.
///
/// This method has a special version when invoked in tests, as we do not want `proc_macro_crate` to
/// be searching the macro crate for the `pinocchio` dependency.
fn find_pinocchio_crate(provided_name: Option<Ident>) -> Result<Ident> {
    if let Some(name) = provided_name {
        return Ok(name);
    }

    match crate_name("pinocchio") {
        Ok(FoundCrate::Itself) => Ok(Ident::new("pinocchio", Span::call_site())),
        Ok(FoundCrate::Name(name)) => Ok(Ident::new_raw(&name, Span::call_site())),
        Err(err) => Err(Error::new(
            Span::call_site(),
            format!(
                "Failed to find a name for the `pinocchio` crate to use.\n\
                 You can either specify it in the `entrypoint! {{}}` macro invocation as a \
                 `pinocchio_crate` argument value, like this:\n\
                 \n\
                 entrypoint! {{\n\
                 \x20   pinocchio_crate: pinocchio,\n\
                 \x20   ...\n\
                 }}\n\
                 \n\
                 Or you need to add `pinocchio` to your Cargo.toml and make sure that \
                 `proc_macro_crate` can find it.  Error from `proc_macro_crate`:\n\
                 {err}"
            ),
        )),
    }
}

#[cfg(test)]
/// Special version of `find_pinocchio_crate` that avoids `proc_macro_crate`.
fn find_pinocchio_crate(provided_name: Option<Ident>) -> Result<Ident> {
    if let Some(name) = provided_name {
        return Ok(name);
    }

    Ok(Ident::new("pinocchio_in_tests", Span::call_site()))
}

#[cfg(test)]
mod tests {
    use super::analyze;

    use crate::{entrypoint::parse::parse, test_utils::assert_macro_error};

    use quote::quote;

    #[test]
    fn single_mut_account() {
        let ast = parse(quote! {
            instruction_type: Instruction,
            program_id: program_id;

            Instruction::Setup => |metadata: Pin<&mut Account<T>>| {
                setup(metadata)
            }
        })
        .unwrap();
        let actual = analyze(ast).unwrap();
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
    }

    #[test]
    fn single_ref_account() {
        let ast = parse(quote! {
            instruction_type: Instruction,
            program_id: program_id;

            Instruction::Setup => |metadata: &Account<T>| {
                setup(metadata)
            }
        })
        .unwrap();
        let actual = analyze(ast).unwrap();
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
    }

    #[test]
    fn missing_program_id() {
        let ast = parse(quote! {
            instruction_type: Instruction;

            Instruction::Setup => |metadata: Pin<&mut Account>| {
                setup(metadata)
            }
        })
        .unwrap();
        let actual = analyze(ast);
        assert_macro_error(
            "`instruction_type` is missing from the argument list",
            actual,
            ":: core :: compile_error ! { \"`program_id` is a required argument\" }",
        );
    }

    #[test]
    fn missing_instruction_type() {
        let ast = parse(quote! {
            program_id: program_id;

            Instruction::Setup => |metadata: Pin<&mut Account>| {
                setup(metadata)
            }
        })
        .unwrap();
        let actual = analyze(ast);
        assert_macro_error(
            "`instruction_type` is missing from the argument list",
            actual,
            ":: core :: compile_error ! { \"`instruction_type` is a required argument\" }",
        );
    }

    #[test]
    fn single_account_with_attributes() {
        let ast = parse(quote! {
            instruction_type: Instruction,
            program_id: program_id;

            Instruction::Setup => |
                #[signer]
                metadata: Pin<&mut Account>
            | {
                setup(metadata)
            }
        })
        .unwrap();
        let actual = analyze(ast);
        assert_macro_error(
            "Account attributes are not supported at the moment",
            actual,
            ":: core :: compile_error ! { \"Account attributes are not supported\" }",
        );
    }

    #[test]
    fn account_and_data_parser() {
        let ast = parse(quote! {
            instruction_type: Instruction,
            program_id: program_id;

            Instruction::NewUser => |metadata: Pin<&mut Account<T>>; name: &str| {
                new_user(metadata, name)
            }
        })
        .unwrap();
        let actual = analyze(ast).unwrap();
        assert_eq!(actual.cases.len(), 1);
        assert_eq!(actual.cases[0].account_parsers.len(), 1);
        assert_eq!(actual.cases[0].data_parsers.len(), 1);
    }

    #[test]
    fn account_and_data_parser_with_attributes() {
        let ast = parse(quote! {
            instruction_type: Instruction,
            program_id: program_id;

            Instruction::NewUser => |
                metadata: Pin<&mut Account<T>>;
                #[max_len(50)]
                name: &str,
            | {
                new_user(metadata, name)
            }
        })
        .unwrap();
        let actual = analyze(ast);
        assert_macro_error(
            "Data parser attributes are not supported at the moment",
            actual,
            ":: core :: compile_error ! { \"Data parser attributes are not supported\" }",
        );
    }
}
