use super::analyze::{AccountParser, DataParser, InstructionBody, InstructionCase, Parameters};

use crate::analyze::Model;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens as _};
use syn::{spanned::Spanned as _, Ident, Pat, Type};

pub fn codegen(
    Model {
        params:
            Parameters {
                pinocchio_crate,
                instruction_type,
                program_id_var_name,
            },
        cases,
    }: Model,
) -> TokenStream {
    let max_num_accounts = cases
        .iter()
        .map(|case| case.account_parsers.len())
        .max()
        .unwrap_or(0);

    let instruction_cases = create_instruction_cases(
        &pinocchio_crate,
        &program_id_var_name,
        max_num_accounts,
        cases,
    );

    quote! {
        #[no_mangle]
        pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
            // SAFETY: `input` is the program entrypoint data.
            match unsafe { entrypoint_impl(input) } {
                Ok(()) => 0,
                Err(err) => err.into(),
            }
        }

        /// Helper to convert `ProgramError` into `u64`, as multiple calls return `ProgramError`.
        ///
        /// # Safety
        ///
        /// `input` must point to bytes that hold a valid encoding of a program entrypoint data.
        unsafe fn entrypoint_impl(
            input: *mut u8,
        ) -> Result<(), #pinocchio_crate::program_error::ProgramError> {
            let mut accounts = [::core::mem::MaybeUninit::uninit(); #max_num_accounts];
            let mut account_references = [::core::mem::MaybeUninit::uninit(); #max_num_accounts];
            // SAFETY: `input` is provided by the VM, and so it expected to be correct.
            // `input` outlives all the references in `accounts`.
            let #pinocchio_crate::entrypoint_v2::ParseProgramInputResult {
                num_accounts,
                instruction_data,
                program_id: #program_id_var_name,
            } = unsafe {
                #pinocchio_crate::entrypoint_v2::parse_program_input::<#max_num_accounts>(
                    &mut accounts,
                    &mut account_references,
                    // SAFETY: `input` is required to be a valid encoding of the program entrypoint
                    // data, and so it is not null.
                    unsafe { ::core::ptr::NonNull::new_unchecked(input) },
                )?
            };

            // Make sure `instruction_type` implements `ProgramDataParser`.
            // let _: &#pinocchio_crate::ProgramDataParser = #instruction_type
            // Attach span to this expression that points to `#instruction_type`.

            let (instruction, instruction_data) =
                <#instruction_type as #pinocchio_crate::entrypoint_v2::ProgramDataParser>
                    ::parse(instruction_data)
                    .map_err(::core::convert::Into::<
                        #pinocchio_crate::program_error::ProgramError>::into)?;

            match instruction {
                #( #instruction_cases )*
            }
        }
    }
}

fn create_instruction_cases(
    pinocchio_crate: &Ident,
    program_id_var_name: &Ident,
    max_num_accounts: usize,
    cases: Vec<InstructionCase>,
) -> Vec<TokenStream> {
    let mut res = Vec::with_capacity(cases.len());
    for InstructionCase {
        instruction_pat,
        account_parsers,
        data_parsers,
        body,
    } in cases
    {
        let expected_num_accounts = account_parsers.len();
        let run_account_parsers =
            create_account_parsers(pinocchio_crate, max_num_accounts, &account_parsers);
        let run_data_parsers = create_data_parsers(pinocchio_crate, &data_parsers);

        let body = create_instruction_body(
            program_id_var_name,
            &instruction_pat,
            &account_parsers,
            &data_parsers,
            body,
        );

        res.push(quote! {
            #instruction_pat => {
                // TODO This should be an equality check, with the parser accepting a `..` at the
                // end of the account parser list in order to allow extra unused accounts.
                if usize::from(num_accounts) < #expected_num_accounts {
                    // TODO Is this the best error?  What programs normally return when an account
                    // is missing?
                    return Err(
                        #pinocchio_crate::program_error::ProgramError::InvalidInstructionData);
                }

                #( #run_account_parsers )*

                #( #run_data_parsers )*

                #body
            }
        });
    }

    res
}

fn create_account_parsers(
    pinocchio_crate: &Ident,
    max_num_accounts: usize,
    account_parsers: &[AccountParser],
) -> Vec<TokenStream> {
    let mut res = vec![];

    for (
        account_idx,
        AccountParser {
            variable_pat,
            account_type,
        },
    ) in (0u8..).zip(account_parsers)
    {
        // TODO Write tests that verify that correct mask is produced:
        // 1. When there are no accounts.
        // 2. When there is 1 account.
        // 3. When there are several accounts and they can reference each other.
        // 4. When there are several accounts and they can not reference each other.
        let compute_reference_mask = create_reference_mask_expression(
            pinocchio_crate,
            account_parsers,
            account_idx,
            account_type,
        );

        res.push(quote! {
            // SAFETY
            //
            // `accounts` and `account_references` is an output from `parse_program_input()`.
            //
            // `account_idx` is below `max_num_accounts` as we computed `max_num_accounts` as the
            // maximum length of any of the `account_parsers`.
            //
            // `account_references`/`accounts` content should be correct, as long as the input data
            // is correct and `parse_program_input()` works correctly.
            //
            // `input` is either provided by the VM or the burden of not moving the input is on the
            // caller, so the `Pin` non-movability should also be satisfied.
            let #variable_pat = unsafe {
                #compute_reference_mask

                <#account_type as #pinocchio_crate::entrypoint_v2::ProgramAccountParser>
                    ::parse::<#max_num_accounts>(
                        &mut accounts,
                        &account_references,
                        allowed_references_mask,
                        #account_idx,
                    )
                    .map_err(::core::convert::Into::<
                        #pinocchio_crate::program_error::ProgramError>::into)?
            };
        });
    }

    res
}

/// Produces a bit mask that has `1` in bits that are valid reference targets based on the specified
/// account types, when referenced by account with index `account_idx` and type `account_type`.
///
/// As references can only point to previous accounts in the list, a mask will only have bits set in
/// positions smaller than `account_idx`.
fn create_reference_mask_expression(
    pinocchio_crate: &Ident,
    account_parsers: &[AccountParser],
    account_idx: u8,
    account_type: &Type,
) -> TokenStream {
    let mask_bits = (0u8..account_idx).zip(account_parsers).filter_map(
        |(
            reference_target_idx,
            AccountParser {
                variable_pat: _,
                account_type: reference_target_type,
            },
        )| {
            if account_idx == reference_target_idx {
                return None;
            }

            let bit_shift = u64::from(reference_target_idx);
            Some(quote! {
                {
                    use #pinocchio_crate::entrypoint_v2::CanReferenceScore;
                    let from_to_score =
                        #pinocchio_crate::entrypoint_v2::ComputeCanReference::<
                            #account_type,
                            #reference_target_type
                        >::dispatcher().score();
                    let to_from_score =
                        #pinocchio_crate::entrypoint_v2::ComputeCanReference::<
                            #reference_target_type,
                            #account_type
                        >::dispatcher().score();

                    u64::from(from_to_score.saturating_add(to_from_score) > 0)
                        << #bit_shift
                }
            })
        },
    );

    let mut all_bits_combined = TokenStream::new();

    for next_bit in mask_bits {
        if all_bits_combined.is_empty() {
            all_bits_combined.extend(next_bit);
        } else {
            all_bits_combined.extend(quote! { | #next_bit });
        }
    }

    if all_bits_combined.is_empty() {
        quote! {
            let allowed_references_mask = 0;
        }
    } else {
        quote! {
            let allowed_references_mask = #all_bits_combined;
        }
    }
}

fn create_data_parsers(pinocchio_crate: &Ident, data_parsers: &[DataParser]) -> Vec<TokenStream> {
    let mut res = vec![];

    for DataParser {
        variable_pat,
        data_type,
    } in data_parsers
    {
        res.push(quote! {
            let (#variable_pat, instruction_data) =
                <#data_type as #pinocchio_crate::entrypoint_v2::ProgramDataParser>
                    ::parse(instruction_data)
                    .map_err(::core::convert::Into::<
                        #pinocchio_crate::program_error::ProgramError>::into)?;
        });
    }

    res
}

fn create_instruction_body(
    program_id_var_name: &Ident,
    instruction_pat: &Pat,
    account_parsers: &[AccountParser],
    data_parsers: &[DataParser],
    body: InstructionBody,
) -> TokenStream {
    let implied_fn = match body {
        InstructionBody::Explicit(body) => return body.into_token_stream(),
        InstructionBody::Implied(implied_fn) => implied_fn,
    };

    let account_parser_var_names = account_parsers.iter().map(
        |AccountParser {
             variable_pat,
             account_type: _,
         }| variable_pat,
    );
    let data_parser_var_names = data_parsers.iter().map(
        |DataParser {
             variable_pat,
             data_type: _,
         }| variable_pat,
    );
    let arg_names = account_parser_var_names.chain(data_parser_var_names);

    quote_spanned! {instruction_pat.span()=>
        #implied_fn ( #program_id_var_name, #( #arg_names ),* )
    }
}

#[cfg(test)]
mod tests {
    use super::codegen;

    use crate::entrypoint::analyze::{Model, Parameters};

    use syn::{
        parse::{ParseStream, Parser},
        parse_quote, ItemFn, Result,
    };

    fn top_level_fns_parser(input: ParseStream) -> Result<Vec<ItemFn>> {
        let mut items = Vec::new();
        while !input.is_empty() {
            items.push(input.parse::<ItemFn>()?);
        }
        Ok(items)
    }

    #[test]
    fn output_is_function_item() {
        let model = Model {
            params: Parameters {
                pinocchio_crate: parse_quote! { pinocchio_v1 },
                instruction_type: parse_quote! { InstructionType },
                program_id_var_name: parse_quote! { program_id },
            },
            cases: vec![],
        };
        let code = codegen(model);

        // println!("D: code: {code:?}");
        assert!(top_level_fns_parser.parse2(code).is_ok());
    }
}
