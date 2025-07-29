//! Some accounts are built in and have a predefined structure.  They are called sysvar accounts,
//! and generally have rules that are different from the rules for the general purpose accounts.
//!
//! In particular, sysvar accounts have certain addresses, and this is how one checks for their
//! "type".  Plus the are always initialized - the VM will populate them.
//!
//! So, a lot of the machinery that deals with the type checking, initialization checking and
//! content can be simplified for the sysvar accounts.
//!
//! This module provides tools for describing sysvar accounts.
//!
//! * [`SysvarAccount<T>`] is a bit like [`OwnAccount<T>`]/[`Account<T>`] - it is a sysvar account
//!   that has been verified during the parsing process.
//! * [`SysvarAccountContent`] is like [`AccountContent`], a trait to be implemented for types that
//!   describe sysvar account content.

use core::{
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    pin::Pin,
    ptr::NonNull,
};

use static_assertions::assert_eq_align;

use crate::{
    entrypoint_v2::{
        parse_single_account_no_references, parse_single_account_with_references, CanReference,
        ProgramAccountParser,
    },
    program_error::ProgramError,
    pubkey::Pubkey,
};

use super::{AbiAccountHeader, AccountCommon};

/// Indicates that an account passed to the program holds a system variable.
///
/// This type is similar to the [`Account`] type for general purpose accounts.  Sysvar account
/// address verification
/// the account address needs to happen via the [`SysvarAccount::require_address()`]
/// method, before account can be accessed by converting it into a reference to
/// [`SysvarAccountChecked`].
///
/// These accounts have slightly different rules on how they are checked and interacted with.  Rules
/// for duplicate references can also be refined, when we know it is a sysvar account.  And thus, it
/// makes sense to provide a different account type for them.
#[derive(Debug)]
#[repr(transparent)]
pub struct SysvarAccount<T: SysvarAccountContent + ?Sized> {
    header: AbiAccountHeader,
    _data: PhantomData<T>,
    _data_is_pinned: PhantomPinned,
}

/// An alias for the [`SysvarAccount<T>`] that hides the `Pin<&mut>` wrapper, reducing visual noise
/// from the usage site.
// TODO It would be nice to specify constraints on `T` explicitly here as in `T:
// SysvarAccountContent + ?Sized`, but it is unsupported as of now.
// See https://github.com/rust-lang/rust/issues/112792
pub type SysvarAccountMut<'a, T> = Pin<&'a mut SysvarAccount<T>>;

impl<T: SysvarAccountContent> SysvarAccount<T> {
    /// `header` can be structurally projected, and this is an projection of it.
    pub fn header(&self) -> &AbiAccountHeader {
        &self.header
    }

    /// `header` should not be moved, as it contains the data length for the bytes that follow the
    /// header itself.  So `header` is projected structurally.
    ///
    /// https://doc.rust-lang.org/std/pin/index.html#choosing-pinning-to-be-structural-for-field
    pub fn header_mut(self: Pin<&mut Self>) -> Pin<&mut AbiAccountHeader> {
        // SAFETY: Inner function does not move any data, and returned `header` projection is also
        // wrapped in `Pin` preventing it from being moved.
        unsafe { self.map_unchecked_mut(|account| &mut account.header) }
    }

    pub fn content(&self) -> &T {
        // SAFETY: `SysvarAccount<T>` can only be constructed for accounts that have addresses
        // matching `<T as SysvarAccountContent>::EXPECTED_ADDRESS`.  So we rely on the VM providing
        // correct data, and on `T` only implementing `SysvarAccountContent` due to it being binary
        // compatible with the VM provided data.
        unsafe { &*self.header.data_slice().as_ptr().cast::<T>() }
    }

    pub fn content_mut(self: Pin<&mut Self>) -> &mut T {
        // SAFETY: `SysvarAccount<T>` can only be constructed for accounts that have addresses
        // matching `<T as SysvarAccountContent>::EXPECTED_ADDRESS`.  So we rely on the VM providing
        // correct data, and on `T` only implementing `SysvarAccountContent` due to it being binary
        // compatible with the VM provided data.
        //
        // It is safe to return `&mut T`, as the contained `T` can be moved, as it should not
        // contain references to the account header or the account itself.
        unsafe { &mut *self.header_mut().data_slice_mut().as_mut_ptr().cast::<T>() }
    }
}

// `&SysvarAccount` can only reference other accounts that have the same type.
impl<C: SysvarAccountContent> CanReference<&SysvarAccount<C>> for &SysvarAccount<C> {
    fn can_reference() -> i8 {
        1
    }
}

// `&mut SysvarAccount` can not reference any other types - weight is `-2`.
impl<FromC: SysvarAccountContent, To> CanReference<To> for &mut SysvarAccount<FromC> {
    fn can_reference() -> i8 {
        -2
    }
}

impl<T: SysvarAccountContent + ?Sized> AccountCommon for SysvarAccount<T> {
    #[inline(always)]
    fn is_signer(&self) -> bool {
        self.header.is_signer != 0
    }

    #[inline(always)]
    fn is_writable(&self) -> bool {
        self.header.is_writable != 0
    }

    #[inline(always)]
    fn is_executable(&self) -> bool {
        self.header.is_executable != 0
    }

    #[inline(always)]
    fn resize_delta(&self) -> i32 {
        self.header.resize_delta()
    }

    #[inline(always)]
    fn address(&self) -> &Pubkey {
        // We may not need `Pubkey` to have an alignment of 1, but it has it currently anyways.
        assert_eq_align!(Pubkey, u8);
        &self.header.address
    }

    #[inline(always)]
    fn owner(&self) -> &Pubkey {
        // We may not need `Pubkey` to have an alignment of 1, but it has it currently anyways.
        assert_eq_align!(Pubkey, u8);
        &self.header.owner
    }

    #[inline(always)]
    fn lamports(&self) -> u64 {
        self.header.lamports()
    }

    #[inline(always)]
    fn data_len(&self) -> u64 {
        self.header.data_len()
    }
}

// SAFETY: By implementing `SysvarAccountContent`, `T` indicates that it can be directly constructed
// from a block of memory holding valid accounts bytes.
unsafe impl<'account, T: SysvarAccountContent + ?Sized> ProgramAccountParser<'_, 'account>
    for &'account SysvarAccount<T>
where
    T: 'account,
{
    type Error = ProgramError;

    /// Constructs an `&SysvarAccount<T>` instance.  It will make sure that the account
    /// address matches `SysvarAccountContent::EXPECTED_ADDRESS`.
    #[inline(always)]
    unsafe fn parse<const MAX_NUM_ACCOUNTS: usize>(
        accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
        account_references: &[MaybeUninit<u8>; MAX_NUM_ACCOUNTS],
        allowed_references_mask: u64,
        account_idx: u8,
    ) -> Result<Self, Self::Error> {
        parse_single_account_with_references::<Self, MAX_NUM_ACCOUNTS, _>(
            accounts,
            account_references,
            allowed_references_mask,
            account_idx,
            parse_account,
        )
    }
}

fn parse_account<'account, T: SysvarAccountContent + ?Sized, const MAX_NUM_ACCOUNTS: usize>(
    accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_idx: u8,
) -> Result<&'account SysvarAccount<T>, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: VM ABI: Account header is correct.
    let header = unsafe { account.as_ref() };

    if header.address != T::EXPECTED_ADDRESS {
        return Err(ProgramError::InvalidArgument);
    }

    // SAFETY: `&SysvarAccount<T>` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.
    Ok(unsafe { account.cast::<SysvarAccount<T>>().as_ref() })
}

// SAFETY: By implementing `SysvarAccountContent`, `T` indicates that it can be directly constructed
// from a block of memory holding valid accounts bytes.
unsafe impl<'account, T: SysvarAccountContent + ?Sized> ProgramAccountParser<'_, 'account>
    for Pin<&'account mut SysvarAccount<T>>
where
    T: 'account,
{
    type Error = ProgramError;

    /// Constructs an `&SysvarAccount<T>` instance.  This will check that the account is writable, and
    /// that the account address matches `SysvarAccountContent::EXPECTED_ADDRESS`.
    ///
    /// `Pin<&'accounts mut SysvarAccount<T>>` does not allow for data sharing.
    #[inline(always)]
    unsafe fn parse<const MAX_NUM_ACCOUNTS: usize>(
        accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
        account_references: &[MaybeUninit<u8>; MAX_NUM_ACCOUNTS],
        // No references are allowed, so this value is irrelevant.
        _allowed_references_mask: u64,
        account_idx: u8,
    ) -> Result<Self, Self::Error> {
        parse_single_account_no_references::<Self, MAX_NUM_ACCOUNTS, _>(
            accounts,
            account_references,
            account_idx,
            parse_account_mut,
        )
    }
}

#[inline(always)]
fn parse_account_mut<'account, T: SysvarAccountContent + ?Sized, const MAX_NUM_ACCOUNTS: usize>(
    accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_idx: u8,
) -> Result<Pin<&'account mut SysvarAccount<T>>, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: VM ABI: Account header is correct.
    let header = unsafe { account.as_ref() };

    if header.is_writable == 0 {
        return Err(ProgramError::Immutable);
    }

    if header.address != T::EXPECTED_ADDRESS {
        return Err(ProgramError::InvalidArgument);
    }

    // SAFETY: `&mut SysvarAccount<T>` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.
    //
    // Responsibility for making sure that the `Pin::new_unchecked()` call is safe is put on the
    // caller of the `parse()` method.
    Ok(unsafe { Pin::new_unchecked(account.cast::<SysvarAccount<T>>().as_mut()) })
}

/// This is a counterpart of the [`AccountContent`] trait for the general purpose accounts.
///
/// Sysvar accounts have requirements that are quite different from those defined in
/// [`AccountContent`], essentially, they just require to have a certain predefined address.
pub trait SysvarAccountContent {
    /// Address an account must have to be considered a sysvar account of the `Self` type.
    const EXPECTED_ADDRESS: Pubkey;
}
