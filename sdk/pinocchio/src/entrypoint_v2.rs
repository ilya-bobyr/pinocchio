//! Tools to describe the program entrypoint, along with some parsing of the instructions and
//! account information.
//!
//! Most programs have very similar structure in how they interpret the `data` and the list of
//! accounts they receive.  Common convention is that the first byte or the first 4 bytes of the
//! `data` selects an "instruction" within the program, and the list of accounts has a fixed meaning
//! for each instruction.  In most case, for a given instruction, the list of accounts must have a
//! known length.  Instruction also assigns meaning the accounts in the list.
//!
//! Sometimes instructions can accept optional accounts, or a variable number of accounts, that all
//! have the same "type".
//!
//! Macros and functions in this module assume the above, and try to remove as much of the
//! boilerplate from your program definition as possible.  While still giving you the flexibility to
//! define your instructions in any way you want.
//!
//! Another very common thing that all Solana programs need to define is the panic handler and the
//! memory allocator.  The SDK will provide a few common solutions.  And you may provide your own
//! implementations for any specialized cases.

use core::{mem::MaybeUninit, ptr::NonNull, slice};

use crate::{
    account::AbiAccountHeader, account_info::MAX_PERMITTED_DATA_INCREASE, hint::likely, log,
    program_error::ProgramError, pubkey::Pubkey, BPF_ALIGN_OF_U128,
};

use static_assertions::{assert_eq_align, assert_eq_size};

mod can_reference;
mod program_account_parser;
mod program_data_parser;

pub use can_reference::CanReference;
pub use program_account_parser::{
    parse_single_account_no_references, parse_single_account_with_references, ProgramAccountParser,
};
pub use program_data_parser::ProgramDataParser;

// TODO Should this import be `#[doc(hidden)]`?
pub use can_reference::{impl_CanReference, CanReferenceScore, ComputeCanReference};

/// Print details on the program input accounts.  Useful when debugging account alignment/address
/// issues.
const LOG_ACCOUNTS: bool = false;

// Program entrypoint.
//
// TODO I would need to write a macro (most likely a proc-macro) that would parse an entrypoint
// description, and will generate an account parser, that would dispatch individual instruction
// execution to the provided callbacks.
//
// TODO I think `&mut Account` might need to be `Pin<&mut Account>`.  The problem is that we do not
// want to allow `Account` instances to be moved.  Need to double check this.
// ```rust
// #[no_mangle]
// pub unsafe extern "C" fn entrypoint(_input: *mut u8) -> u64 {
//     todo!();
// }
// ```

/// This function does an initial parsing of the program input data.
/// It finds positions of all the accounts, as well as the program instruction data.
///
/// When the program is invoked, we want to know which instruction does it suppose to execute.  And
/// the instruction is normally provided as the first byte, or the fist several bytes of the program
/// instruction data.
///
/// So we need to parse the input data to a certain level of depth, in order to makes sense of the
/// account list, as it is generally defined by the instruction been invoked.  Instruction will also
/// determine how to parse instruction data after the instruction ID.
///
/// One inconvenience related to the input data parsing is that it starts with a variable sized
/// account list.  With individual elements of variable sizes.  It would be much better if the
/// offsets of all the main elements, or at least the first level structure would be at fixed
/// offsets, or at offsets specified in the input.
///
/// `input` is serialized in `program-runtime/src/serialization.rs`, and the end result is:
///
///  * 4 bytes:
///    number of accounts.
///
///    **bikeshedding** Why use 4 bytes?  There could not be more than around 38 accounts, so, for
///    now, a single byte would be enough.  And we do use 1 byte fields below.  Plus, the duplicate
///    reference is using a single byte, so unless we change how the duplicates are referenced, we
///    would not be able to support more than 256 accounts, even if we increase the transaction
///    size.
///
/// * account bytes:
///   Accounts can be defined in one of the two formats.
///
///   First format is a full account description, in case this account appears in the account list
///   for the first time.  Exact details are in `account/abi_header.rs`.
///
///   Second format is for accounts that are repeated in the account list.  In this case, 8 bytes
///   are used.  And the first byte is not 256, but instead an index of the first occurrence of this
///   account in the list.  The very first account can not be a duplicate, so account list must
///   start with 256.  7 bytes after the first are all `0`.
///
/// * 8 bytes: size of the instruction data.
///   (As the instruction data is part of the transaction, this can not be more than 1200 bytes
///   right now.  Why do we use u64?)
///
/// *  instruction data bytes:  As many as specified in the "size of the instruction data" field.
///
/// *  32 bytes: program id.  (No alignment here?)
///
/// It would be so much better to start with the program id, followed by the instruction data length,
/// then the instruction data, and then the accounts list.
///
/// This function will try to do as little work as possible, in order to find the start of the
/// instruction data.  But as it needs to parse pretty much all the other parts of the input data,
/// it stores all the found details.  During normal execution, all of it will be used later.  So,
/// for a successful execution, it should not really matter.  Maybe, though unlikely, for some
/// optimizations the compiler could have performed.  We do not care to optimize much for failed
/// executions, I would imagine.
///
/// # Arguments
///
/// * `accounts` - only some of the values will be initialized.  Specifically, those for which
///   `account_references` is `u8::MAX`, and that have index less than
///   `min(MAX_NUM_ACCOUNTS, ParseProgramInputResult::num_accounts)`.  Each account that was passed
///   into the program will be present here.  Accounts that are duplicates will be left
///   uninitialized, and `account_references` will specify the index of the account to use instead.
///
/// * `account_references` - Accounts that are explicitly provided to the program will have
///   corresponding values in the `account_references` set set to `u8::MAX`.  Accounts that are
///   duplicates of accounts with lower indices will have values other than `u8::MAX.  The value is
///   the index of the full account, current account is a duplicate of.  Unlike the `accounts`
///   slice, this slice is initialized with no gaps, up to the index of `min(MAX_NUM_ACCOUNTS,
///   ParseProgramInputResult::num_accounts)`.
///
/// * `input` - data provided by the runtime to the program entry point.  All the returned data that
///   uses the `'input` lifetime references data directly from this pointer.
///
/// # Safety
///
/// `input` must point to a block of bytes that represent a valid serialization of the program
/// accounts, instruction data and a program ID.  Returned pointers and references that use the
/// `'input` lifetime all reference back to the `input` data.  The compiler will not be able to make
/// sure that `input` outlives the returned values.
pub unsafe fn parse_program_input<'res, 'input, const MAX_NUM_ACCOUNTS: usize>(
    accounts: &'res mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_references: &'res mut [MaybeUninit<u8>; MAX_NUM_ACCOUNTS],
    input: NonNull<u8>,
) -> Result<ParseProgramInputResult<'input>, ProgramError> {
    // TODO Switch to `NonNull` when the MSRV is 1.80.0 or above.  A considerable portion of the
    // `NonNull` API stabilized in 1.80.0.
    let mut input: *mut u8 = input.as_ptr();

    assert_eq_size!(usize, u64);
    let num_accounts =
        // SAFETY: VM ABI: `input` has at least 8 more bytes that hold the number of accounts.
        u64::from_le_bytes(unsafe { input.cast::<[u8; size_of::<u64>()]>().read() }) as usize;
    input = input.add(size_of::<u64>());

    // TODO: It is possible to unroll this loop to improve the program CU usage.  But it will
    // generate more instructions, making the program longer.  Not really sure which way the
    // optimization should go here.
    //
    // TODO I see that Accounts v2 uses more CUs than original Pinocchio parsing in p-token. Could
    // it be due to the original doing the loop unrolling here?  Need to check.
    for account_idx in 0..num_accounts {
        // SAFETY: VM ABI: there should be at least `num_accounts` accounts in the input data.
        let prev_account_ref = unsafe { input.read() };

        if LOG_ACCOUNTS {
            log::sol_log("parse_program_input.1: input, account_idx, prev_account_ref:");
            log::sol_log_64(
                input as u64,
                account_idx as u64,
                prev_account_ref as u64,
                0,
                0,
            );
        }

        if prev_account_ref != u8::MAX {
            // No additional checking is performed here, as we assume the VM is providing us with
            // valid data.  Duplicate account reference should always point to a full account that
            // is already in the list.
            if likely(account_idx < MAX_NUM_ACCOUNTS) {
                account_references[account_idx].write(prev_account_ref);
            }

            // `accounts` is not updated on purpose, as the caller should be able to tell this
            // `account` value is not initialized, as the `account_references` values is not
            // `u8::MAX`.

            // SAFETY: VM ABI: there should be at least 8 bytes holding the previous account reference.
            input = unsafe { input.add(8) };
        } else {
            if likely(account_idx < MAX_NUM_ACCOUNTS) {
                account_references[account_idx].write(u8::MAX);
            }

            let account = input.cast::<AbiAccountHeader>();

            if LOG_ACCOUNTS {
                log::sol_log("parse_program_input.2: account:");
                unsafe { &*account }.log();
            }

            // SAFETY: VM ABI: `input` (aka `account`) must point to a valid `AbiAccountHeader`
            // instance.
            if likely(account_idx < MAX_NUM_ACCOUNTS) {
                accounts[account_idx].write(unsafe { NonNull::new_unchecked(account) });
            }

            assert_eq_size!(usize, u64);
            // SAFETY: VM ABI: `input` (aka `account`) must point to a valid `AbiAccountHeader`
            // instance.
            let account_data_len = unsafe { account.read() }.data_len() as usize;

            // The extra `u64` at the end is the `rent_epoch`.  As accounts are not allowed to be
            // non-rent exempt any more, this field will always be `u64::MAX`.  And VM ignores this
            // value upon termination.  So there seems to be no value in reading it.
            // SAFETY: VM ABI: `input` must point to a complete account memory block.
            input = unsafe {
                input.add(
                    size_of::<AbiAccountHeader>()
                        + account_data_len
                        + MAX_PERMITTED_DATA_INCREASE
                        + size_of::<u64>(),
                )
            };
            // `align_offset` may actually return `usize::MAX`, but we assume that the VM has
            // provided us with a valid input, in which case this step should never fail.
            // Note that this increment can not be combined with the previous one, as we need to use
            // an updated value of `input` when we are computing the necessary alignment offset.
            // SAFETY: VM ABI: `input` must point to a complete account memory block.
            input = unsafe { input.add(input.align_offset(BPF_ALIGN_OF_U128)) };
        }
    }

    assert_eq_size!(usize, u64);
    let instruction_data_len =
        // SAFETY: VM ABI: `input` has at least 8 more bytes that hold the subsequent data length.
        u64::from_le_bytes(unsafe { input.cast::<[u8; size_of::<u64>()]>().read() }) as usize;
    input = input.add(size_of::<u64>());

    // SAFETY: VM ABI: `input` must extend at least `instruction_data_len` bytes further on.
    let instruction_data = unsafe { slice::from_raw_parts(input, instruction_data_len) };
    input = input.add(instruction_data_len);

    assert_eq_align!(Pubkey, u8);
    // SAFETY: VM ABI: `input` points at a `Pubkey` encoding the program ID at this point.
    // Alignment checked above.
    let program_id = unsafe { &*(input as *const Pubkey) };

    let Ok(num_accounts) = u8::try_from(num_accounts) else {
        return Err(ProgramError::InvalidAccountData);
    };

    Ok(ParseProgramInputResult {
        num_accounts,
        instruction_data,
        program_id,
    })
}

/// Output of a [`parse_program_input`] function.
///
/// # Lifetimes
///
/// * `'input` - lifetime of the input data that was provided to the [`parse_program_input`]
///   function.  All references and pointers in this struct use this lifetime, as they all reference
///   back to the `input` data.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ParseProgramInputResult<'input> {
    pub num_accounts: u8,
    pub instruction_data: &'input [u8],
    pub program_id: &'input Pubkey,
}
