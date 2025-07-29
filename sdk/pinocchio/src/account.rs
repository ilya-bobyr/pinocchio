//! While fundamentally all accounts are identical, there are several distinct ways in which a
//! program interprets accounts it receives.  In order to make it more explicit in the code, there
//! is a set of concrete types that represent different ways of interpreting an account.  Examples
//! would be:
//!   * [`NewAccount`] - account that is expected to be empty for this instruction invocation.  So
//!     the account will need to be initialized before it can be accessed.
//!   * [`OwnAccount`] - an ownership check will be required, before account data can be accessed.
//!
//! [`NewAccount`]: account::NewAccount
//! [`OwnAccount`]: account::OwnAccount
//!
//! Mostly, the differences only matter during the initial verification of the accounts provided for
//! a particular program execution.  After the initial verification is complete, with few
//! exceptions, accounts are treated in a similar fashion.  The inner account data can obviously be
//! different, and it is quite important what kind of data is stored there.  But differences between
//! a `NewAccount` and an `OwnAccount` become irrelevant.
//!
//! [`Account<T: AccountContent>`] is the type that is used for "accounts that have passed the
//! initial verification".  With few exceptions, most of the program logic should be expressed using
//! [`Account<T: AccountContent>`].  With `T` representing the structure of the data stored in the
//! account bytes.  When account data is irrelevant for the program operation, [`AnyAccount`] should
//! be used.  It also represents an account that have passed the necessary initial verification, but
//! prevents access to the account bytes.  Still allowing access to all the common account
//! proprieties, such as the signer flag, via the [`AccountCommon`] trait.
//!
//! [`AccountCommon`]: account::AccountCommon.
//!
//! Programs are not expected to create new account instances, except in a process of parsing the
//! entrypoint input data.
//!
//! Use the [`entrypoint_v2::entrypoint!`] macro to do most of the parsing for you.
//!
//! TODO Tests will need to create new accounts.  Provide testing tools that help with that.

use core::{
    marker::{PhantomData, PhantomPinned},
    pin::Pin,
};

use crate::pubkey::Pubkey;

use static_assertions::assert_eq_align;

pub mod abi_account_header;
pub mod account_common;
pub mod account_content;
pub mod any_account;
pub mod initializable;
pub mod new_account;
pub mod own_account;
pub mod sysvar_account;

pub use abi_account_header::AbiAccountHeader;
pub use account_common::{AccountCommon, AccountCommonMut};
pub use account_content::{
    account_content_cast_bytes, account_content_cast_bytes_mut, AccountContent,
};
pub use any_account::{AnyAccount, AnyAccountMut};
pub use initializable::{initialize_bytes_with_sized_self, Initializable};
pub use new_account::{NewAccount, NewAccountMut};
pub use own_account::{OwnAccount, OwnAccountMut};
pub use sysvar_account::{SysvarAccount, SysvarAccountContent};

/// Describes an account that has passed initial verification stage and is ready to be accessed by
/// the rest of the program logic.  Account bytes should be interpreted as an instance of type `T`.
/// See [`AccountContent`] for the details of this interpretation.
///
/// Accounts are normally provided by the virtual machine as global objects, and are never
/// constructed or destroyed by the program code.  As such, an Pinocchio program is not expected to
/// operate on `Account<T>` values by value.  Instead, it normally only deals with references to
/// `Account<T>`.
///
/// By default, `Account` uses `[u8]` as the data type, which works for any kind of account.
/// Though, if you do not need to access account bytes it is better to use [`AnyAccount`].
#[derive(Debug)]
#[repr(transparent)]
pub struct Account<T: AccountContent + ?Sized> {
    header: AbiAccountHeader,
    _data: PhantomData<T>,
    _data_is_pinned: PhantomPinned,
}

/// An alias for the [`Account<T>`] that hides the `Pin<&mut>` wrapper, reducing visual noise from
/// the usage site.
// TODO It would be nice to specify constraints on `T` explicitly here as in `T: AccountContent +
// ?Sized`, but it is unsupported as of now.
// See https://github.com/rust-lang/rust/issues/112792
pub type AccountMut<'a, T> = Pin<&'a mut Account<T>>;

impl<T: AccountContent + ?Sized> Account<T> {
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

    pub fn content(&self) -> T::AsContent<'_> {
        // SAFETY: `Account<T>` can only be constructed for accounts that have the right type, and
        // that are initialized, satisfying `AccountContent::content_ref()` safety requirements.
        unsafe { T::content(self.header.data_slice()) }
    }

    pub fn content_mut(self: Pin<&mut Self>) -> T::AsContentMut<'_> {
        // SAFETY: `Account<T>` can only be constructed for accounts that have the right type, and
        // that are initialized, satisfying `AccountContent::content_ref()` safety requirements.
        //
        // It is safe to return `&mut T`, as the contained `T` can be moved, as it should not
        // contain references to the account header or the account itself.
        unsafe { T::content_mut(self.header_mut().data_slice_mut()) }
    }
}

// TODO As the VM aligns every account on an 8 byte boundary, all reads below are actually aligned.
// Do we want to rely on this?  Maybe check it in the debug build, but assume things are properly
// aligned in the release?  Not really sure if our current BPF bytecode will be different.  But it
// could be if we migrate to a different VM and the ABI layout changes.  Would be nice to make sure
// the compiler would automatically point here, if any of those things happen.
impl<T: AccountContent + ?Sized> AccountCommon for Account<T> {
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

impl<T: AccountContent + ?Sized> AccountCommonMut for Account<T> {
    #[inline(always)]
    fn set_owner(self: Pin<&mut Self>, value: &Pubkey) {
        // We may not need `Pubkey` to have an alignment of 1, but it has it currently anyways.
        assert_eq_align!(Pubkey, u8);
        // Assuming `&Account` is constructed from a valid serialization of a VM account,
        // this write should be safe.
        //
        // TODO Considering we found that comparing `Pubkey` one `u64` at a time is more efficient,
        // it might be similarly more efficient to assign them by interpreting the `Pubkey` as 4
        // `u64` values.
        self.header_mut().owner = *value;
    }

    #[inline(always)]
    fn set_lamport(self: Pin<&mut Self>, value: u64) {
        self.header_mut().set_lamports(value)
    }
}

// NOTE When I'll be implementing a shared mutable account type, it would be useful to know, which
// fields of the account the VM care about when the program terminates.
//
// Relevant code is in `deserialize_parameters_aligned` in `program-runtime/src/serialization.rs`.
//
// `borrow_state`, `is_signer`, `is_writable`, `executable`, `resize_delta`, and `key` are ignored.
//
// If we move `is_signer`, `is_writable` and `executable` into 3 bits elsewhere, the first 6 bytes
// of the account can be freely used.  `resize_delta` also does not need 4 bytes it is allocated.
// As the account size can not change by more than 10,240
// (`solana_account_info::MAX_PERMITTED_DATA_INCREASE`) within a single instruction, so we can
// easily use 2 bytes to store the length change.  16 bits can encode values between -32,768 and
// 32,767.
//
// As the account size is limited to 10MB, there is no good reason to use 8 bytes for `data_len` to
// store the account length either.  24 bits is enough, which is 3 bytes.  And then 2 bytes can be
// used to store the account size change.  With 3 bytes still available.  The problem here is that
// the VM will actually read all 8 bytes and treat them as a new account length.  So we would need
// to guarantee that we would overwrite the length difference with zeros on all exit paths.
// Technically, this is not very complicated it seems.  But the 6 bytes we get from the first fields
// might already be enough.
//
// `owner`, `lamports`, and `data_len` matter - they are read by the VM and the account is updated
// to match the new values of these fields.
