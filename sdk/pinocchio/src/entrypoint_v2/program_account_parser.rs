//! When a program entrypoint is processing program data, we want to simplify the process to the
//! point when accounts can be processed in sequence.
//!
//! Unfortunately, there are a few rough edges, and so the `ProgramAccountParser` trait is both
//! `unsafe` and is very tightly connected to the output of the `parse_program_input()` function.
//!
//! Still, a trait provides a possible extension point, as well as reduces the amount of hidden
//! logic, produced by the `parse_and_process_instruction!` macro.

use core::{mem::MaybeUninit, ptr::NonNull};

use crate::{account::AbiAccountHeader, hint::unlikely, program_error::ProgramError};

/// Given an output from the [`parse_program_input()`], constructs an instance of an account object,
/// for account with a particular index, given by `account_idx`.
///
/// `accounts`, `account_references` and `num_accounts` are outputs of the [`parse_program_input()`]
/// - see there for details.
///
/// [`parse_program_input()`]: `super::parse_program_input()`
///
/// # Safety
///
/// This trait can only be implemented for types that can be directly constructed from a block of
/// memory that holds a valid account bytes, as provided by the VM.
///
/// Some accounts types (such as [`SharedAccountRef`] can modify account bytes as part of their type
/// construction.  As the caller of the [`parse()`] method needs to guarantee that each account
/// index is constructed from only once, implementation of this type can rely on this.
///
/// Which means that, for example, when parsing a non-reference account, it is safe to access it in
/// an exclusive manner, as this parsing will only happen once.  Yet, implementation needs to
/// consider any possible shared references, if the account type at hand allows sharing.  For
/// example, [`SharedAccountRef`], when parsing a reference account, should expect the reference
/// target to be already modified by the [`SharedAccountRef`] parse process.
///
/// While `'account` lifetime should be always shorter than the `'input_accounts` lifetime, we can
/// not add an explicit `'input_accounts: 'account` constraint.  It would cause the borrow checker
/// to block the input accounts after parsing a single mutable account (returned `&'account mut
/// Self` will block any usage of `'input_accounts`).  As a result, the borrow checker is disabled
/// and the implementation needs to be careful to avoid any overlapping mutable access.
///
// Rather then putting constraints on `Self`, I've tried an alternative that looks like this:
//
// ```rust
// pub unsafe trait ProgramAccountParser
// {
//     type ParsesInto<'accounts>: 'accounts;
//
//     unsafe fn parse<const MAX_NUM_ACCOUNTS: usize>(
//         accounts: &'accounts mut [MaybeUninit<NonNull<u8>>; MAX_NUM_ACCOUNTS],
//         account_references: &[MaybeUninit<u8>; MAX_NUM_ACCOUNTS],
//         num_accounts: u8,
//         account_idx: u8,
//     ) -> Result<Self::ParsesInto<'accounts>, ProgramError>;
// }
// ```
//
// But while the trait itself compiles, I failed to write an implementation for it for
// `NewAccount<T>`, with `ParsesInto<'accounts> = &'accounts NewAccount<T>`.
//
// The problem is that the compiler requires a `T: 'accounts` constraint.  But if I add it at the
// `ParsesInto` level, it claims that the trait implementation is more constrained than the trait.
// Forcing me to move the `'accounts` lifetime to the trait level.  As this is the only location
// where I can then use it in at the implementation spot, to introduce the `T: 'accounts`
// constraint.  At which point, I think, the version below seems more natural.
//
// An intermediate `ParsesInto` is a bit wired, as it allows more flexibility than seems necessary.
// I do not think that I want `&NewAccount<T>` to parse into anything but `&NewAccount<T>`.
pub unsafe trait ProgramAccountParser<'input_accounts, 'account>
where
    Self: Sized + 'account,
{
    /// Errors that can be produced when trying to construct an account object.
    ///
    /// As [`parse_with_references()`] is used by the [`entrypoint!()`] macro, produced error must
    /// be convertible into [`ProgramError`] automatically.  Otherwise parser written by the macro
    /// would not be able to propagate them.  Thus, the `Into<ProgramError>` constraint is required.
    ///
    /// `From<ProgramError>` is necessary for the default implementation of
    /// `parse_with_references()`, as it needs to construct an error in case a reference is provided
    /// to an account that can not be referenced by the current account type.
    /// `ProgramError::InvalidAccountData`.
    // TODO An alternative to requiring `From<ProgramError>` would be a method (or a const value)
    // that returns `Self::Error` to be used when the default implementation of
    // `parse_with_references()` needs to signal an invalid reference.  A method would allow for
    // arbitrary types.  While a const would require `Self::Error` to be `Clone`.
    type Error: Into<ProgramError> + From<ProgramError>;

    /// Constructs an instance of `Self` holding an account reference from the results of a
    /// `parse_program_input()` invocation.
    ///
    /// When this method is implemented, it should almost always we marked with `#[inline(always)]`
    /// to make sure unnecessary checks are dissolved at compile time.
    ///
    /// This method can also be overwritten by account types that perform initialization of the
    /// non-reference accounts.  Such as [`&SharedAccountRef`].
    ///
    /// There are several helper functions that implement this method for different parsing
    /// strategies:
    ///
    /// * [`parse_single_account_with_references`] - for parsing types holding a single account
    ///   reference that allow account duplicate references.
    ///
    /// * [`parse_single_account_no_references`] - for parsing types holding a single account
    ///   reference that do not allow account duplicate references.
    ///
    /// Default implementation will parse exactly one account, allowing references as specified by
    /// the `allowed_references_mask`, and forwarding the actual parsing to the `parse()` method.
    ///
    /// If this account type allows duplicate references, this method default implementation can be
    /// used.  If a reference was provided, the default implementation will resolve it and check if
    /// `allowed_references_mask` is set to `1` at the corresponding position.  Next it then forward
    /// to the [`parse()`] implementation.
    ///
    /// Account types that do not allow duplicates, should implement this method and produce an
    /// error if a reference is selected.  Default implementation will also prevent references as
    /// the `allowed_references_mask` will hold only zeroes.  It is only a matter of the optimizer
    /// removing unnecessary checks one way or another.
    ///
    /// # Safety
    ///
    /// `accounts`, and `account_references` must represent a valid output from a
    /// [`parse_program_input()`] invocation.
    /// `account_idx` must be below `MAX_NUM_ACCOUNTS`.
    /// `account_references[account_idx]` must be initialized.
    /// `account_references[account_idx]` must be an index of an initialized element in `accounts`,
    /// which is strictly lower than `account_idx`, or be `u8::MAX`, in the latter case
    /// `accounts[account_idx]` must be initialized.
    ///
    /// Depending on the type of `Self`, the caller might be required to only construct a single
    /// value out of each element of the `accounts` array, by only calling this method with distinct
    /// values of `account_idx`.  This restriction is lifted for account types that are designed to
    /// work as shared references, see [`&SharedAccountRef`].
    ///
    /// As `account_idx` is mapped via the `account_references`, accounts types that can not work as
    /// references need to prevent overlapped usages by overwriting this method, constructing `Self`
    /// only if `account_references[account_idx]` is `u8::MAX.  This needs to be a runtime check by
    /// the trait method implementation.  The caller is not responsible for it.
    ///
    /// `'input_accounts` and `'account` lifetimes are disconnected from the borrow checker
    /// standpoint, but in reality, `'account` must have a shorter lifetime than `'input_accounts`.
    ///
    /// When `Self` is `Pin<T>`, the caller is also responsible for making sure that the
    /// corresponding `accounts` element is not moved until it is dropped.  For account data
    /// provided by the VM this happens automatically, as it lives for the duration of the program.
    /// But for tests this is something that needs to be ensured by the caller.
    // TODO Link the doc above to the doc-comment of [`&SharedAccount`].
    unsafe fn parse<const MAX_NUM_ACCOUNTS: usize>(
        accounts: &'input_accounts mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
        account_references: &'input_accounts [MaybeUninit<u8>; MAX_NUM_ACCOUNTS],
        allowed_references_mask: u64,
        start_account_idx: u8,
    ) -> Result<Self, Self::Error>;
}

/// Helper that implements [`ProgramAccountParser::parse()`], allowing duplicate account references,
/// with accordance to the `allowed_references_mask`, calling the specified callback once to parse
/// the account further, after possible account duplicate reference has been resolved.
///
/// You want to use this implementation for read-only shared account references, such as
/// `&AnyAccount`.
///
/// # Safety
///
/// Safety requirements are exactly the same as in [`ProgramAccountParser::parse()`]
///
// TODO Link the doc above to the doc-comment of [`&SharedAccount`].
#[inline(always)]
pub unsafe fn parse_single_account_with_references<
    'input_accounts,
    'account,
    T,
    const MAX_NUM_ACCOUNTS: usize,
    Parse,
>(
    accounts: &'input_accounts mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_references: &'input_accounts [MaybeUninit<u8>; MAX_NUM_ACCOUNTS],
    allowed_references_mask: u64,
    start_account_idx: u8,
    parse: Parse,
) -> Result<T, T::Error>
where
    T: ProgramAccountParser<'input_accounts, 'account>,
    T::Error: From<ProgramError>,
    Parse: FnOnce(
        /* accounts: */
        &'input_accounts mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
        /* account_idx: */ u8,
    ) -> Result<T, T::Error>,
{
    let mut account_idx = start_account_idx;
    debug_assert!(
        account_idx != u8::MAX,
        "`u8::MAX` is not a valid account index."
    );
    debug_assert!(
        usize::from(account_idx) <= MAX_NUM_ACCOUNTS,
        "MAX_NUM_ACCOUNTS is {MAX_NUM_ACCOUNTS}, account_idx of {account_idx} is out of bounds."
    );

    // SAFETY: `account_references[account_idx]` is initialized by the method preconditions.
    let reference_idx = unsafe { account_references[account_idx as usize].assume_init() };
    if unlikely(reference_idx != u8::MAX) {
        debug_assert!(
            reference_idx < account_idx,
            "Account references can only point to previous accounts.\n\
             account_references[{account_idx}] has a value of {reference_idx}."
        );
        debug_assert!(
            usize::from(reference_idx) <= MAX_NUM_ACCOUNTS,
            "MAX_NUM_ACCOUNTS is {MAX_NUM_ACCOUNTS}, account_references[{account_idx}] \
             contains {reference_idx}, which is out of bounds."
        );
        if cfg!(debug_assertions) {
            // SAFETY: `account_references` has all the values initialized sequentially up to
            // the last provided account number.  And references only point to previous
            // accounts, meaning that the reference target must also be initialized.
            let target_reference_idx =
                unsafe { account_references[reference_idx as usize].assume_init() };
            assert!(
                target_reference_idx == u8::MAX,
                "account_references[{account_idx}] points to account {reference_idx}, but \
                 account_references[{reference_idx}] is {target_reference_idx}, not u8::MAX. \
                 Only a single level of indirection is allowed."
            );
        }

        if allowed_references_mask & (1 << reference_idx) == 0 {
            // TODO Is this the most accurate error for this case?
            return Err(T::Error::from(ProgramError::InvalidAccountData));
        }

        account_idx = reference_idx;
    }

    parse(accounts, account_idx)
}

/// Helper that implements [`ProgramAccountParser::parse()`], disallowing duplicate account
/// references, calling the specified callback once after checking on account references are
/// present.
///
/// You want to use this implementation for exclusive shared account references, such as
/// `&mut AnyAccount`.
///
/// # Safety
///
/// Safety requirements are exactly the same as in [`ProgramAccountParser::parse()`]
///
// TODO Link the doc above to the doc-comment of [`&SharedAccount`].
#[inline(always)]
pub unsafe fn parse_single_account_no_references<
    'input_accounts,
    'account,
    T,
    const MAX_NUM_ACCOUNTS: usize,
    Parse,
>(
    accounts: &'input_accounts mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_references: &'input_accounts [MaybeUninit<u8>; MAX_NUM_ACCOUNTS],
    account_idx: u8,
    parse: Parse,
) -> Result<T, T::Error>
where
    T: ProgramAccountParser<'input_accounts, 'account> + 'account,
    T::Error: From<ProgramError>,
    Parse: FnOnce(
        /* accounts: */
        &'input_accounts mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
        /* account_idx: */ u8,
    ) -> Result<T, T::Error>,
{
    // SAFETY: `account_references[account_idx]` is initialized by the method preconditions.
    if unlikely(unsafe { account_references[account_idx as usize].assume_init() } != u8::MAX) {
        return Err(T::Error::from(ProgramError::AccountBorrowFailed));
    }

    debug_assert!(
        account_idx != u8::MAX,
        "`u8::MAX` is not a valid account index."
    );
    debug_assert!(
        usize::from(account_idx) <= MAX_NUM_ACCOUNTS,
        "MAX_NUM_ACCOUNTS is {MAX_NUM_ACCOUNTS}, account_idx of {account_idx} is out of bounds."
    );

    parse(accounts, account_idx)
}
