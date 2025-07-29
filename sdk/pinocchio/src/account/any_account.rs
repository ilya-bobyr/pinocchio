use core::{marker::PhantomPinned, mem::MaybeUninit, pin::Pin, ptr::NonNull};

use static_assertions::assert_eq_align;

use crate::{
    entrypoint_v2::{
        parse_single_account_no_references, parse_single_account_with_references, CanReference,
        ProgramAccountParser,
    },
    program_error::ProgramError,
    pubkey::Pubkey,
};

use super::{AbiAccountHeader, AccountCommon, AccountContent, OwnAccount};

/// Used for accounts when the account data is not going to be accessed.  Allows access to the
/// common account properties, stored in the account header, such as lamport balance, owner and
/// address.
///
/// Can reference or be referenced by accounts that use Rust lifetimes, when use as a shared
/// reference.  In other words, for example, if the program accounts are specified as
/// `&OwnAccount<T>, `&AnyAccount`, it is allowed to pass the same account to the program twice.
#[derive(Debug)]
#[repr(transparent)]
// This type should not implement `Copy` as it is not movable.  Not sure why
// `missing_copy_implementations` is flagging it.  Am I missing something?
#[allow(missing_copy_implementations)]
pub struct AnyAccount {
    header: AbiAccountHeader,
    _data_is_pinned: PhantomPinned,
}

/// An alias for the [`AnyAccount`] that hides the `Pin<&mut>` wrapper, reducing visual noise from
/// the usage site.
pub type AnyAccountMut<'a> = Pin<&'a mut AnyAccount>;

impl AnyAccount {
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
    pub fn require_owner(&self, value: &Pubkey) -> Result<&Self, ProgramError> {
        if self.owner() == value {
            Ok(self)
        } else {
            // TODO Not clear if `ProgramError::IllegalOwner` is a better choice.  I do not
            // understand the difference between these errors.
            Err(ProgramError::InvalidAccountOwner)
        }
    }

    #[inline(always)]
    pub fn require_owner_mut<'account>(
        self: Pin<&'account mut Self>,
        value: &Pubkey,
    ) -> Result<Pin<&'account mut Self>, ProgramError> {
        if self.owner() == value {
            Ok(self)
        } else {
            // TODO Not clear if `ProgramError::IllegalOwner` is a better choice.  I do not
            // understand the difference between these errors.
            Err(ProgramError::InvalidAccountOwner)
        }
    }
}

// `&AnyAccount` can reference other accounts that use Rust lifetimes for access control and are
// shared.
impl CanReference<&AnyAccount> for &AnyAccount {
    fn can_reference() -> i8 {
        1
    }
}

impl<ToC: AccountContent> CanReference<&OwnAccount<ToC>> for &AnyAccount {
    fn can_reference() -> i8 {
        1
    }
}

// `&mut NewAccount` can not reference any other types.  So we give a weight of `-2` for any target.
impl<To> CanReference<To> for &mut AnyAccount {
    fn can_reference() -> i8 {
        -2
    }
}

impl AccountCommon for AnyAccount {
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

// SAFETY: `AnyAccount` blocks access to the account bytes, so there is never going to be any
// conversion of the content.
unsafe impl<'account> ProgramAccountParser<'_, 'account> for &'account AnyAccount {
    type Error = ProgramError;

    /// Constructs an `&AnyAccount` instance.  This does *not* check that the account is
    /// initialized.
    ///
    /// Ownership check is performed by [`AnyAccount::require_owner()`].
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

fn parse_account<'account, const MAX_NUM_ACCOUNTS: usize>(
    accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_idx: u8,
) -> Result<&'account AnyAccount, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: `&AnyAccount` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.  `T::is_initialized()` is `false`.
    Ok(unsafe { account.cast::<AnyAccount>().as_ref() })
}

// SAFETY: `AnyAccount` blocks access to the account bytes, so there is never going to be any
// conversion of the content.
unsafe impl<'account> ProgramAccountParser<'_, 'account> for Pin<&'account mut AnyAccount> {
    type Error = ProgramError;

    /// Constructs an `Pin<&'account mut AnyAccount>` instance.  This will check that the account is
    /// writable, but does *not* check that it is initialized.
    ///
    /// `Pin<&'accounts mut AnyAccount>` do not allow for data sharing.
    ///
    /// Ownership check is performed by [`AnyAccount::require_owner()`].
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

fn parse_account_mut<'account, const MAX_NUM_ACCOUNTS: usize>(
    accounts: &mut [MaybeUninit<NonNull<AbiAccountHeader>>; MAX_NUM_ACCOUNTS],
    account_idx: u8,
) -> Result<Pin<&'account mut AnyAccount>, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: VM ABI: Account header is correct.
    let header = unsafe { account.as_ref() };

    if header.is_writable == 0 {
        return Err(ProgramError::Immutable);
    }

    // SAFETY: `&mut AnyAccount` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.
    //
    // Responsibility for making sure that the `Pin::new_unchecked()` call is safe is put on the
    // caller of the `parse()` method.
    Ok(unsafe { Pin::new_unchecked(account.cast::<AnyAccount>().as_mut()) })
}
