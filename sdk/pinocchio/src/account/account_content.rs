//! Most accounts have a fixed structure that can be described as a struct, making it easier to
//! interact with them.  That is, compared to operating directly on bytes.
//!
//! [`AccountContent`] trait should be implemented for types that describe accounts.  Note that this
//! is different from a type that can be stored in an account as a field or as part of a larger data
//! structure.  Specifically, only types that represent an account as a whole should implement
//! [`AccountContent`].
//!
//! While Solana does not put any restrictions on the way accounts are structured, there is a common
//! convention that a prefix of the account data of a certain length identifies the account type, in
//! the context of the owning program.  While this trait does not enforce this convention, unless
//! you follow it, you may find it hard to construct an IDL for your program.
//!
//! # Account discriminator
//!
//! Common convention is to use a prefix of the account bytes as an identifier of the account type,
//! without the set of accounts owned by the same program.  This prefix can have different length
//! and possible valid values could have different formats.  One common name for this prefix is the
//! "account discriminator".
//!
//! [The Anchor framework](https://www.anchor-lang.com/), uses discriminator that are 8 bytes long,
//! and to store a sha256 hash of the string that consists of a prefix "account:" followed by the
//! account type name.  The account type name here is just the name chosen in the source code of the
//! owning program.  You can find more details in [the Anchor IDL
//! documentation](https://www.anchor-lang.com/docs/basics/idl#discriminators).
//!
//! It feels a bit wasteful to use 8 bytes just for the account type ID, considering that accounts
//! types are also restricted by the owning program.  A single byte with sequential numbers should
//! work equally well.  As Solana charges users for storage it can be worth optimizing at the byte
//! level, at least in case when more than one account instance will be constructed.
//!
//! Pinocchio does not enforce a particular scheme, and allows you to use either the Anchor style
//! account discriminator, a single byte, or something else.  The discriminator format is also
//! defined per account type.
//!
//! Yet, as certain conventions are so common, it does not make sense to rewrite code that
//! implements the same discriminator format over and over again.  Pinocchio provides default
//! implementation if you want to use the prefix bytes of the account data as a discriminator.
//!
//! There are macros that can write an instance of the `AccountContent` trait for your type
//! following one of the common conventions.
//!
//! TODO Implement macros that support Anchor style discriminators and a single byte discriminators.
//! Link them here.
//!
//! ## Common discriminator default implementation
//!
//! A number of [`AccountContent`] methods and [`Initializable::initialize_bytes()`] have default
//! implementations that use the [`DISCRIMINATOR`] associated constant in the [`AccountContent`]
//! instance.  If you write your own version of [`AccountContent`] and/or [`Initializable`] for your
//! account type, you can just provide the desired value of the [`DISCRIMINATOR`] and use the
//! default implementation.  This is a recommended approach as it ensures that the implementations
//! of all of those methods match each other.
//!
//! TODO Will this work for automatic IDL generation?  Would there be another method, responsible
//! for the IDL generation, that would also rely on the `DISCRIMINATOR` value?
//!
//! # Initialization
//!
//! Another convention is the notion of an "initialized" account.  All accounts are pre-filled with
//! zeroes when they are constructed.  But most programs expect certain invariants to hold for any
//! meaningful accounts they control.  An uninitialized account is thus a special state that needs
//! to be initialized before a program will start using this account in any other operations.
//!
//! A discriminator that holds only zero bytes is commonly used to distinguish an uninitialized
//! account from an initialized one.
//!
//! Pinocchio does not enforce a specific initialization pattern, instead trying to be flexible and
//! support common conventions.  If you rely on the non-zero discriminator, the macros that write
//! `AccountContent` implementations for you support that.  Or you can use a different kind of
//! check.  As long as any of the account bytes are non-zero it is initialized from the standpoint
//! of the system program.  But it is inefficient to check all bytes every time.
//!
//! It is still not recommended to deviate from the common conventions, as it would be hard or
//! impossible to describe such accounts in IDLs, and other on-chain and off-chain programs will
//! have to do more work to deal with accounts with non-standard type format.
//!
//! Initialization for each account type can be different, and, if supported, is covered by the
//! [`Initializable`] trait.
//!
//! [`Initializable`]: crate::account::initializable::Initializable
//!
//! Make sure that [`Initializable::initialize()`] is consistent with the check performed by
//! [`AccountContent::is_initialized()`].
//!
//! TODO The discriminator is part of the initialization process.  Maybe there would be a macro that
//! writes common [`NewAccount::initialize()`] instances?  In which case, there might be an overlap
//! with a macro that write the [`AccountContent::is_initialized()`] implementation.
//!
//! # Versioning
//!
//! TODO I want to add versioning as a basic concept.  I think versioning should live outside of the
//! account type.  I think a simple two component version should cover a huge number of use cases,
//! considering the scale of the Solana applications.
//!
//! Essentially, I want to provide a default, easy to use (hopefully almost free to use) solution,
//! where an account version is stored in addition to the account discriminator.  Version can be one
//! or two bytes - still need to think about this trade-off.  Maybe there is a way to automagically
//! switch from one to two bytes?
//!
//! In any case, a version encodes two values: a minor version and a major version.  When reading
//! the account data the rules are:
//!  - Newer minor version indicates backward compatible layout.  New data added at the end, or
//!    previously unused bytes are in use now.  Any of the existing fields did not change their
//!    semantics and any of the newly added fields do not affect semantics of the existing fields.
//!  - Newer major version indicates a backward incompatible layout or a change in the semantics of
//!    any of the fields.  Note that adding a new enum value is backaord incompatible.  The
//!    [`#[non_exhaustive]`] attribute does not help here, as older code would not know how to skip
//!    the value it does not understand.
//!
//! [`#[non_exhaustive]`]: https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute
//!
//! When writing the account data it is necessary for the program to understand the exact meaning of
//! both the minor and major versions.
//!
//! Versioning should be useful as follows.  Program should be always aware of all the versions of
//! it's own account data.  For each account type and each version is should be able to read the
//! content correctly, and, hopefully, upgrade the content to the latest version.  Upgrade being a
//! separate complex topic.
//!
//! For programs that read accounts of other programs (as is customary in Solana), they can read an
//! account data even if minor version is newer.
//!
//! While it is not impossible to describe in text, the above restriction may have a non-trivial
//! impact on the `is_of_type()` method.  Essentially, we might need two methods, now, maybe?
//!
//! Need to think more about it.  Maybe there is some simple solution that we can start using,
//! before we figure out all the tricky details.  I would like to be able to just start with all new
//! accounts allocating a byte or two with a version of `0`.  As adding a version retroactively is
//! actually pretty hard.  When the version is allocated and set, it is much easier to look for
//! solutions in each particular case.  The question is: is there a solution that would work for
//! everyone?  Or a solution that works for a large enough percentage of cases, and can be
//! overwritten in cases that it does not cover?

use crate::program_error::ProgramError;

/// Types that can be stored in accounts as a top level entity.  Manages account lifetime, such as
/// initialization, and type checks.
///
/// # Safety
///
/// There are several rules that need to be followed when implementing methods of this trait.  See
/// individual method doc-comments for those.  Each implementation needs to make sure that the rules
/// are followed, otherwise undefined behavior or security issues may arise, as the callers of the
/// methods rely on all those rules.
pub unsafe trait AccountContent {
    /// Discriminator bytes that should be used for accounts holding `Self`.
    ///
    /// This value is only used by the default implementations of [`AccountContent`] methods, and
    /// the [`Initializable::initialize_bytes()`] method.
    ///
    /// Rather than writing your own version of [`AccountContent`] and/or [`Initializable`] for your
    /// account type, you can just provide the desired value of the [`DISCRIMINATOR`] and use the
    /// default implementation.  This is a recommended approach as it ensures that the
    /// implementations of all of those methods are consistent with each other.
    ///
    /// # Safety - Uniqueness
    ///
    /// It is important to make sure that all accounts for a given program use different values of
    /// the discriminator bytes.  Ideally you probably want to use the same length [`DISCRIMINATOR`]
    /// arrays and use different values for different specific types that describe accounts.
    ///
    /// If you use use `DISCRIMINATOR` arrays that have different length, you need to make sure that
    /// they are different, when cut to the length of the shortest `DISCRIMINATOR` array your
    /// program is using.  Such that account initialized with a longer `Discriminator` does not have
    /// the same prefix as the account initialized with a shorter `Discriminator`.
    ///
    /// For example, if you use both `[u8; 1]` and `[u8; 2]` as the `DISCRIMINATOR` type, you can
    /// not use `0x01` for `[u8; 1]` and `[0x01, 0x00]` for `[u8; 2]`.  As `0x01, 0x00` if viewed as
    /// a `[u8; 1]` it is indistinguishable from `0x01`.
    // TODO It would have been more efficient to encode the discriminator length statically, but it
    // does not seem that I can reference another `const` in the same trait, as in:
    //
    // ```rust
    // const DISCRIMINATOR_LEN: usize;
    // const DISCRIMINATOR: &'static [u8; Self::DISCRIMINATOR_LEN];
    // ```
    //
    // It would be possible to do it using the `typenum`, and `generic-array` crates.
    //
    // https://crates.io/crates/typenum
    // https://crates.io/crates/generic-array
    //
    // `generic-array` summary specifically lists this particular use case.  Still impossible with
    // Rust native const generics.  But I'm not sure what is the deal with using external crates in
    // the Pinocchio public API.
    //
    // `typenum` is a very well known crate in the Rust ecosystem.  So it should be safe to use in
    // the public API.
    //
    // Another alternative design here is to allow the discriminator to be any type that implements
    // `IntoBytes` from `zerocopy`.  Not entirely sure it is providing a lot of value, as user code
    // is unlikely to interact with discriminators directly, and so the ability to use something
    // that is not a fixed size byte array is probably not particularly useful?
    // https://docs.rs/zerocopy/0.8.27/zerocopy/trait.IntoBytes.html
    const DISCRIMINATOR: &'static [u8];

    /// Given the account bytes, checks if the account was initialized.  As the caller can not know,
    /// in a general case what is the minimum length of an account of type `T`, `is_initialized()`
    /// is required to return `ProgramError::AccountDataTooSmall` if `data` is too short for the
    /// check to be meaningful.
    ///
    /// If `is_initialized(data)` returned `Ok(_)` it should be safe to call `is_of_type(data)`.
    ///
    /// Common convention is that the account discriminator is stored in the initial account bytes.
    /// Default implementation of this method uses `DISCRIMINATOR` and assumes that it is recorded
    /// as the prefix bytes of the account data.  Unlike [`is_of_type()`] this method, by default,
    /// just checks that any of the `DISCRIMINATOR` bytes is non-zero.
    ///
    /// You can provide a more efficient implementation - such as checking that one specific byte of
    /// the non-zero bytes in the `DISCRIMINATOR` range is non-zero in the account, for example.
    /// But it will probably be a rather minor gain.
    ///
    fn is_initialized(data: &[u8]) -> Result<bool, ProgramError> {
        debug_assert!(
            Self::DISCRIMINATOR.is_empty(),
            "DISCRIMINATOR should not be empty, as it would match any account."
        );
        debug_assert!(
            Self::DISCRIMINATOR.iter().any(|v| *v != 0),
            "DISCRIMINATOR can not contain only zero bytes.  `is_initialized()` implementation \
             would be incorrect."
        );

        let discriminator_len = Self::DISCRIMINATOR.len();

        if data.len() < discriminator_len {
            return Err(ProgramError::AccountDataTooSmall);
        }

        Ok(data[0..Self::DISCRIMINATOR.len()].iter().any(|v| *v != 0))
    }

    /// Given the account bytes, checks if the account has a discriminator value that matches this
    /// type.  This method must return `false` for uninitialized accounts.  For efficiency reasons,
    /// this method is not expected to perform additional checks, such as size checks.
    ///
    /// For accounts that use discriminators, this should be as simple as checking the discriminator
    /// value.  As the account is owned by the program and only the program can change it's data, an
    /// invalid length is a bug in the program.  Correct length is an invariant identical to all the
    /// other invariants a valid account must adhere to.  And so there is no good reason to check
    /// just one specific invariant every time.
    ///
    /// There are no restrictions on how this method can be implemented, and it can use any logic,
    /// not necessarily checking the account prefix.  But any non-trivial logic might be hard or
    /// impossible to describe in the program IDL.
    ///
    /// If this method returns `true`, it should be the case that calling [`cast()`] and
    /// [`cast_mut()`] is safe.
    ///
    /// # Safety
    ///
    /// `data` was checked with `is_initialized(data)` and it returned `Ok(_)`.
    unsafe fn is_of_type(data: &[u8]) -> bool {
        debug_assert!(
            Self::DISCRIMINATOR.is_empty(),
            "DISCRIMINATOR should not be empty, as it would match any account."
        );
        debug_assert!(
            Self::DISCRIMINATOR.iter().any(|v| *v != 0),
            "DISCRIMINATOR can not contain only zero bytes.  `is_of_type()` implementation would \
             be incorrect."
        );
        debug_assert!(
            data.len() >= Self::DISCRIMINATOR.len(),
            "`is_of_type()` precondition is that `data` contains enough bytes to check for the \
             presence of `Self::DISCRIMINATOR` value."
        );

        &data[..Self::DISCRIMINATOR.len()] == Self::DISCRIMINATOR
    }

    /// Return type for the `content()` method.  This allows account content to be expressed as fat
    /// pointer or as any other data-structure.
    ///
    /// Note that you still should reuse as much of the data in the account as possible.  This is
    /// not a mechanism for serialization/deserialization of the data.
    ///
    /// For types that rely on lifetimes for correctness checks to the account data, `'data`
    /// lifetime must be part of the returned type.
    // TODO Add a default value of `&'data Self` when associated type defaults are stabilized.
    // https://github.com/rust-lang/rust/issues/29661
    // type AsContent<'data> = &'data Self;
    type AsContent<'data>;

    /// This method is used to cast bytes of an account into an instance of the type the account is
    /// expected to hold.
    ///
    /// For the purposes of efficiency, this method should not perform any checks.  It should just
    /// skip the discriminator part of the data, if there is one in the account, and cast the rest
    /// of the bytes into an instance of [`Self::AsContent<'data>`].  In most cases,
    /// [`Self::AsContent<'data>`] is just `&'data Self`, in which case a default implementation of
    /// this method is provided as an [`account_content_cast_bytes()`] function.
    ///
    /// # Safety
    ///
    /// This method can only be called on an account for which [`is_of_type()`] returns `true`.  It
    /// is required that `data` contains bytes that can be `transmute`ed into `Self`.
    ///
    /// Default implementation performs no size checks on `data`, assuming, in addition to the
    /// above, that after skipping `Self::DISCRIMINATOR.len()` bytes, the rest of `data` is a valid
    /// `Self` instance.
    unsafe fn content(data: &[u8]) -> Self::AsContent<'_>;

    /// Return type for the `content_mut()` method.  This allows account content to be expressed as
    /// fat pointer or as any other data-structure.
    ///
    /// Note that you still should reuse as much of the data in the account as possible.  This is
    /// not a mechanism for serialization/deserialization of the data.
    ///
    /// For types that rely on lifetimes for correctness checks to the account data, `'data`
    /// lifetime must be part of the returned type.
    // TODO Add a default value of `&'data Self` when associated type defaults are stabilized.
    // https://github.com/rust-lang/rust/issues/29661
    // type AsContent<'data> = &'data mut Self;
    type AsContentMut<'data>;

    /// This method is used to cast bytes of an account into an instance of the type the account is
    /// expected to hold.
    ///
    /// For the purposes of efficiency, this method should not perform any checks.  It should just
    /// skip the discriminator part of the data, if there is one in the account, and cast the rest
    /// of the bytes into a reference to `Self`.
    ///
    /// # Safety
    ///
    /// This method can only be called on an account for which [`is_of_type()`] returns `true`.  It
    /// is required that `data` contains bytes that can be `transmute`ed into `Self`.
    ///
    /// Default implementation performs no size checks on `data`, assuming, in addition to the
    /// above, that after skipping `Self::DISCRIMINATOR.len()` bytes, the rest of `data` is a valid
    /// `Self` instance.
    // TODO See `cast()` for a note on the `Self: Sized` constraint.
    unsafe fn content_mut(data: &mut [u8]) -> Self::AsContentMut<'_>;
}

/// A common implementation of [`AccountContent::content()`] method, that just skips the
/// [`AccountContent::DISCRIMINATOR`] and cast the rest of the account bytes into `&Self`.
///
/// # Safety
///
/// This function can only be called when `T` can be constructed as a reinterpretation of bytes
/// that were previously written from `T`.
///
/// `data` must be a `T::DISCRIMINATOR` followed by bytes that represent a valid instance of `T`.
#[inline(always)]
pub unsafe fn account_content_cast_bytes<'data, T>(data: &'data [u8]) -> &'data T
where
    T: AccountContent<AsContent<'data> = &'data T>,
{
    debug_assert!(
        T::DISCRIMINATOR.is_empty(),
        "DISCRIMINATOR should not be empty, as it would match any account."
    );

    // As `T` is not required to be `Sized`, we can not call `size_of` on it.
    // In practice in the majority of cases `T` will be sized.
    // TODO Is there a way to specialize `account_content_cast_bytes()` such that this check is
    // performed when `T` is sized?
    //
    // debug_assert!(
    //     data.len() >= T::DISCRIMINATOR.len() + size_of::<T>(),
    //     "`content()` precondition is that `data` contains enough bytes to hold a discriminator \
    //      followed by an instance of `Self`."
    // );

    // SAFETY: `cast` requires `data` to contain a valid instance of `T` after the discriminator as
    // a precondition.  And `T` should be constructable from the bytes contained in `data`.
    unsafe { &*data.as_ptr().add(T::DISCRIMINATOR.len()).cast::<T>() }
}

/// A common implementation of [`AccountContent::content_mut()`] method, that just skips the
/// [`AccountContent::DISCRIMINATOR`] and cast the rest of the account bytes into `&mut Self`.
///
/// # Safety
///
/// This function can only be called when `T` can be constructed as a reinterpretation of bytes
/// that were previously written from `T`.
///
/// `data` must be a `T::DISCRIMINATOR` followed by bytes that represent a valid instance of `T`.
#[inline(always)]
pub unsafe fn account_content_cast_bytes_mut<'data, T>(data: &'data mut [u8]) -> &'data mut T
where
    T: AccountContent<AsContentMut<'data> = &'data mut T>,
{
    debug_assert!(
        T::DISCRIMINATOR.is_empty(),
        "DISCRIMINATOR should not be empty, as it would match any account."
    );

    // As `T` is not required to be `Sized`, we can not call `size_of` on it.
    // In practice in the majority of cases `T` will be sized.
    // TODO Is there a way to specialize `account_content_cast_bytes()` such that this check is
    // performed when `T` is sized?
    //
    // debug_assert!(
    //     data.len() >= T::DISCRIMINATOR.len() + size_of::<T>(),
    //     "`content_mut()` precondition is that `data` contains enough bytes to hold a \
    //      discriminator followed by an instance of `Self`."
    // );

    // SAFETY: `cast` requires `data` to contain a valid instance of `T` after the discriminator as
    // a precondition.  And `T` should be constructable from the bytes contained in `data`.
    unsafe { &mut *data.as_mut_ptr().add(T::DISCRIMINATOR.len()).cast::<T>() }
}

// TODO Not really sure this instance is actually correct.  And an empty account is
// considered initialized - which is incorrect.  `initialize()` does not write down a discriminator
// and `is_initialized()` is not checking it.
//
// `is_of_type()` is also always true...
//
// `AnyAccount` might be a better alternative in case the account data does not need to be accessed.
// And for cases when account needs to be treated as a sequence of bytes, I'm planning to provide a
// `BytesAccount` type instead.
//- /// An implementation for an account that holds bytes.  It trivially accepts all accounts.
//- ///
//- impl AccountContent for [u8] {
//-     type Init = ();
//-
//-     fn is_initialized(_data: &[u8]) -> bool {
//-         true
//-     }
//-
//-     unsafe fn initialize(_state: (), _data: &[u8]) {
//-         panic!("Account<[u8]> is considered trivially initialized");
//-     }
//-
//-     fn is_of_type(_data: &[u8]) -> bool {
//-         true
//-     }
//-
//-     unsafe fn cast(value: &[u8]) -> &Self {
//-         value
//-     }
//-
//-     unsafe fn cast_mut(value: &mut [u8]) -> &mut Self {
//-         value
//-     }
//- }
