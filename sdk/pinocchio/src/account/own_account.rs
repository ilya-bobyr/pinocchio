use core::{
    marker::{PhantomData, PhantomPinned},
    mem::{transmute, MaybeUninit},
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

use super::{AbiAccountHeader, Account, AccountCommon, AccountContent, AnyAccount};

/// Indicates that an account passed to the program needs to be initialized and will require an
/// ownership check, before content can be accessed via a conversion into an [`Account<T>`]
/// instance.
///
/// # Safety
///
/// `OwnAccount<T>` can only be constructed when `T::is_initialized(data)` is `true` for the data of
/// the account.
#[derive(Debug)]
#[repr(transparent)]
pub struct OwnAccount<T: AccountContent + ?Sized> {
    header: AbiAccountHeader,
    _data: PhantomData<T>,
    _data_is_pinned: PhantomPinned,
}

/// An alias for the [`OwnAccount<T>`] that hides the `Pin<&mut>` wrapper, reducing visual noise
/// from the usage site.
// TODO It would be nice to specify constraints on `T` explicitly here as in `T: AccountContent +
// ?Sized`, but it is unsupported as of now.
// See https://github.com/rust-lang/rust/issues/112792
pub type OwnAccountMut<'a, T> = Pin<&'a mut OwnAccount<T>>;

impl<T: AccountContent + ?Sized> OwnAccount<T> {
    #[inline(always)]
    pub fn require_signer(&self) -> Result<&Self, ProgramError> {
        if self.is_signer() {
            Ok(self)
        } else {
            Err(ProgramError::MissingRequiredSignature)
        }
    }

    #[inline(always)]
    pub fn require_signer_mut(self: Pin<&mut Self>) -> Result<Pin<&mut Self>, ProgramError> {
        if self.is_signer() {
            Ok(self)
        } else {
            Err(ProgramError::MissingRequiredSignature)
        }
    }

    #[inline(always)]
    pub fn require_address(&self, value: &Pubkey) -> Result<&Self, ProgramError> {
        if self.address() == value {
            Ok(self)
        } else {
            // TODO The name and/or signature of the method should probably be different to reflect
            // the address check here.
            //
            // Or maybe this is a wrong error code.  But I could not fund a better one.
            // `ProgramError::InvalidAccountData` seems misleading, as the account address is not
            // really part of the account data.
            Err(ProgramError::InvalidSeeds)
        }
    }

    #[inline(always)]
    pub fn require_address_mut<'account>(
        self: Pin<&'account mut Self>,
        value: &Pubkey,
    ) -> Result<Pin<&'account mut Self>, ProgramError> {
        if self.address() == value {
            Ok(self)
        } else {
            // TODO The name and/or signature of the method should probably be different to reflect
            // the address check here.
            //
            // Or maybe this is a wrong error code.  But I could not fund a better one.
            // `ProgramError::InvalidAccountData` seems misleading, as the account address is not
            // really part of the account data.
            Err(ProgramError::InvalidSeeds)
        }
    }

    #[inline(always)]
    pub fn require_owner<'account>(
        &'account self,
        value: &Pubkey,
    ) -> Result<&'account Account<T>, ProgramError> {
        if self.owner() != value {
            // TODO Not clear if `ProgramError::IllegalOwner` is a better choice.  I do not
            // understand the difference between these errors.
            return Err(ProgramError::InvalidAccountOwner);
        }

        // SAFETY: `&OwnAccount<T>` and `&Account<T>` have the same alignment and layout, and
        // contain the same data.  So `transmute()` should be safe.
        Ok(unsafe { transmute::<&'account Self, &'account Account<T>>(self) })
    }

    #[inline(always)]
    pub fn require_owner_mut<'account>(
        self: Pin<&'account mut Self>,
        value: &Pubkey,
    ) -> Result<Pin<&'account mut Account<T>>, ProgramError> {
        if self.owner() != value {
            // TODO Not clear if `ProgramError::IllegalOwner` is a better choice.  I do not
            // understand the difference between these errors.
            return Err(ProgramError::InvalidAccountOwner);
        }

        // SAFETY: Nested function does not move from it's argument.
        Ok(unsafe {
            self.map_unchecked_mut(|account| {
                // SAFETY: `&mut OwnAccount<T>` and `&mut Account<T>` have the same alignment and
                // layout, and contain the same data.  So `transmute()` should be safe.
                transmute(account)
            })
        })
    }
}

// `&OwnAccount` can reference other accounts that use Rust lifetimes for access control and are
// shared.
impl<FromC: AccountContent, ToC: AccountContent> CanReference<&OwnAccount<ToC>>
    for &OwnAccount<FromC>
{
    fn can_reference() -> i8 {
        1
    }
}

impl<FromC: AccountContent> CanReference<&AnyAccount> for &OwnAccount<FromC> {
    fn can_reference() -> i8 {
        1
    }
}

// `&mut NewAccount` can not reference any other types.  So we give a weight of `-2` for any target.
impl<FromC: AccountContent, To> CanReference<To> for &mut OwnAccount<FromC> {
    fn can_reference() -> i8 {
        -2
    }
}

impl<T: AccountContent + ?Sized> AccountCommon for OwnAccount<T> {
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

// SAFETY: By implementing `AccountContent`, `T` indicates that it can be directly constructed from
// a block of memory holding valid accounts bytes.
unsafe impl<'account, T: AccountContent + ?Sized> ProgramAccountParser<'_, 'account>
    for &'account OwnAccount<T>
where
    T: 'account,
{
    type Error = ProgramError;

    /// Constructs an `&OwnAccount<T>` instance.  This will check that the account is initialized,
    /// according to `AccountContent::is_initialized()`.
    ///
    /// Ownership check is performed by [`OwnAccount::require_owner()`].
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

fn parse_account<'account, T: AccountContent + ?Sized, const MAX_NUM_ACCOUNTS: usize>(
    accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_idx: u8,
) -> Result<&'account OwnAccount<T>, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: VM ABI: Account header is correct.
    let header = unsafe { account.as_ref() };
    let data = header.data_slice();

    if !T::is_initialized(data)? {
        return Err(ProgramError::UninitializedAccount);
    }

    // SAFETY: `&OwnAccount<T>` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.  `T::is_initialized()` is `false`.
    Ok(unsafe { account.cast::<OwnAccount<T>>().as_ref() })
}

// SAFETY: By implementing `AccountContent`, `T` indicates that it can be directly constructed from
// a block of memory holding valid accounts bytes.
unsafe impl<'account, T: AccountContent + ?Sized> ProgramAccountParser<'_, 'account>
    for Pin<&'account mut OwnAccount<T>>
where
    T: 'account,
{
    type Error = ProgramError;

    /// Constructs a `Pin<&'account mut OwnAccount<T>>` instance.  This will check that the account
    /// is writable, and that it is initialized, according to `AccountContent::is_initialized()`.
    ///
    /// Ownership check is performed by [`OwnAccount::require_owner()`].
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
fn parse_account_mut<'account, T: AccountContent + ?Sized, const MAX_NUM_ACCOUNTS: usize>(
    accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_idx: u8,
) -> Result<Pin<&'account mut OwnAccount<T>>, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: VM ABI: Account header is correct.
    let header = unsafe { account.as_ref() };

    if header.is_writable == 0 {
        return Err(ProgramError::Immutable);
    }

    let data = header.data_slice();
    if !T::is_initialized(data)? {
        return Err(ProgramError::UninitializedAccount);
    }

    // SAFETY: `&mut OwnAccount<T>` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.
    //
    // Responsibility for making sure that the `Pin::new_unchecked()` call is safe is put on the
    // caller of the `parse()` method.
    Ok(unsafe { Pin::new_unchecked(account.cast::<OwnAccount<T>>().as_mut()) })
}
