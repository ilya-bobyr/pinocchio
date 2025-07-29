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

use super::{AbiAccountHeader, Account, AccountCommon, AccountContent, Initializable};

/// Indicates that an account passed to the program needs to be uninitialized and will require
/// initialization before it can be converted into an [`Account<T>`] instance and used in the rest
/// of the program logic.
///
/// # Safety
///
/// `NewAccount<T>` can only be constructed when `T::is_initialized(data)` is `false` for the data
/// of the account.
#[derive(Debug)]
#[repr(transparent)]
pub struct NewAccount<T: AccountContent + ?Sized> {
    header: AbiAccountHeader,
    _data: PhantomData<T>,
    _data_is_pinned: PhantomPinned,
}

/// An alias for the [`NewAccount<T>`] that hides the `Pin<&mut>` wrapper, reducing visual noise
/// from the usage site.
// TODO It would be nice to specify constraints on `T` explicitly here as in `T: AccountContent +
// ?Sized`, but it is unsupported as of now.
// See https://github.com/rust-lang/rust/issues/112792
pub type NewAccountMut<'a, T> = Pin<&'a mut NewAccount<T>>;

impl<T: AccountContent + ?Sized> NewAccount<T> {
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

impl<T: Initializable> NewAccount<T> {
    /// `header` should not be moved, as it contains the data length for the bytes that follow the
    /// header itself.  So `header` is projected structurally.
    ///
    /// https://doc.rust-lang.org/std/pin/index.html#choosing-pinning-to-be-structural-for-field
    #[inline(always)]
    fn header_mut(self: Pin<&mut Self>) -> Pin<&mut AbiAccountHeader> {
        // SAFETY: Inner function does not move any data, and returned `header` projection is also
        // wrapped in `Pin` preventing it from being moved.
        unsafe { self.map_unchecked_mut(|account| &mut account.header) }
    }

    /// Initializes account bytes to hold a discriminator and an instance of `T`, as defined by the
    /// [`AccountContent`] instance for `T`.  Returns an [`Account<T>`] as an initialized account.
    ///
    /// Expects account to have enough space to store the discriminator and an instance of `T` in
    /// the already allocated space.
    ///
    /// TODO It would probably make sense to provide another initialization function that would
    /// extend the account size.  And/or another initialization function that might also adjust the
    /// account balance, to cover rent for the necessary space.
    #[inline(always)]
    pub fn initialize<'init, 'account>(
        mut self: Pin<&'account mut Self>,
        state: T::Init<'init>,
    ) -> Result<Pin<&'account mut Account<T>>, ProgramError>
    where
        Self: Sized,
    {
        let mut header = self.as_mut().header_mut();
        let data = header.as_mut().data_slice_mut();

        {
            // SAFETY: `initialize_bytes()` is required to not set any bytes of `data` into an
            // uninitialized state.  Making this `transmute()` safe.
            let data_as_uninit: &mut [MaybeUninit<u8>] = unsafe { transmute(data) };
            // SAFETY: `is_initialized(data)` is checked in the `<NewAccount<T> as
            // ProgramAccountParser>::parse()` and `NewAccount<T>` can only be constructed if
            // `is_initialized()` returned `false`.
            unsafe { T::initialize_bytes(state, data_as_uninit)? }
        }

        // SAFETY: Nested function does not move from it's argument.
        let initialized_account: Pin<&mut Account<T>> = unsafe {
            self.map_unchecked_mut(|account| {
                // SAFETY: `NewAccount<T>` and `Account<T>` have the same alignment and layout, and
                // contain the same data.  So `transmute()` should be safe.
                //
                // TODO It would be nice to encode the fact that `&mut NewAccount<T>` can be
                // transmuted into `&mut Account<T>` in code.  I was thinking that maybe
                // `NewAccount` could contain `Account` as the only field, but then it breaks the
                // assumption that `Account` represents an initialized account.
                transmute(account)
            })
        };

        Ok(initialized_account)
    }
}

// `&mut NewAccount` can not reference any other types.  So we give a weight of `-2` for any target.
impl<To, C: AccountContent> CanReference<To> for &mut NewAccount<C> {
    fn can_reference() -> i8 {
        -2
    }
}

impl<T: AccountContent + ?Sized> AccountCommon for NewAccount<T> {
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
    for &'account NewAccount<T>
where
    T: 'account,
{
    type Error = ProgramError;

    /// Constructs a `&NewAccount<T>` instance from a read-only account.  This will check that the
    /// account is not initialized, according to `AccountContent::is_initialized()`.
    ///
    /// Initialization is performed by the [`NewAccount::initialize()`] method.  But it is only
    /// callable on a `Pin<&mut NewAccount>`, so it is impossible to initialize a `&NewAccount`
    /// instance.  Meaning, this method is probably useless in practice.
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
) -> Result<&'account NewAccount<T>, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: VM ABI: Account header is correct.
    let header = unsafe { account.as_ref() };
    let data = header.data_slice();

    if T::is_initialized(data)? {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // SAFETY: `&NewAccount<T>` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.  `T::is_initialized()` is `false`.
    Ok(unsafe { account.cast::<NewAccount<T>>().as_ref() })
}

// SAFETY: By implementing `AccountContent`, `T` indicates that it can be directly constructed from
// a block of memory holding valid accounts bytes.
unsafe impl<'account, T: AccountContent + ?Sized> ProgramAccountParser<'_, 'account>
    for Pin<&'account mut NewAccount<T>>
where
    T: 'account,
{
    type Error = ProgramError;

    /// Constructs a `Pin<&'account mut NewAccount<T>>` instance.  This will check that the account
    /// is writable, and that it is *not* initialized, according to
    /// `AccountContent::is_initialized()`.
    ///
    /// Initialization is performed by the [`NewAccount::initialize()`] method.
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
) -> Result<Pin<&'account mut NewAccount<T>>, ProgramError> {
    // SAFETY: According to method preconditions `accounts[account_idx]` must be initialized.
    let account = unsafe { accounts[account_idx as usize].assume_init() };

    // SAFETY: VM ABI: Account header is correct.
    let header = unsafe { account.as_ref() };

    if header.is_writable == 0 {
        return Err(ProgramError::Immutable);
    }

    let data = header.data_slice();
    if T::is_initialized(data)? {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // SAFETY: `&mut NewAccount<T>` is designed to be constructed as a cast from
    // `NonNull<AbiAccountHeader>`.
    //
    // Responsibility for making sure that the `Pin::new_unchecked()` call is safe is put on the
    // caller of the `parse()` method.
    Ok(unsafe { Pin::new_unchecked(account.cast::<NewAccount<T>>().as_mut()) })
}
