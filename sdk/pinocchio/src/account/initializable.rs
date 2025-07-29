use core::mem::{size_of, MaybeUninit};

use crate::program_error::ProgramError;

use super::AccountContent;

/// For a given account type (see [`AccountContent`]) the program may or may not know how to
/// initialize new accounts.  For accounts that the program owns and can create it would have the
/// corresponding code.  But for accounts it only accesses in read-only mode, it does not need to
/// know how to initialize a new account.
///
/// This trait should be implemented for account types that the program can initialize.
///
/// Note that a program should still be able to tell an initialized account from an un-initialized
/// one, as only initialized accounts should be read.  And so the `is_initialized()` check is part
/// of the [`AccountContent`] trait.
///
/// See [`AccountContent`] for general overview about types that represent account data.
///
/// [`AccountContent`]: crate::account::account_content::AccountContent
// TODO Maybe look for a better name?
pub trait Initializable: AccountContent {
    /// Arguments needed by the [`initialize()`] in order to initialize account bytes to represent a
    /// valid state of `Self`.  For types that require no external information, a unit ([`()`]) is
    /// the right choice.
    ///
    /// `'source` allows for references in the initialization data.  It does not mean that those are
    /// allowed in the account data. But passing references to existing data into the initialization
    /// logic can be more efficient.
    // TODO Add [`()`]` as a default value when associated type defaults are supported.
    // ```rust
    // type Init<'source> = ();
    // ```
    // https://github.com/rust-lang/rust/issues/29661
    type Init<'init>;

    // TODO Now that `AccountContent` supports change of the target type from `Self`, I think I
    // would need to rethink what `Initializable` would look like.  The change for `AccountContent`
    // was done to support types that can change their size dynamically, such as a log, which would
    // be something like a `Vec<LogEntry>` represented by a fat pointer.
    //
    // `Initializable` would then need to be made more complex as well.  It needs to use
    // `AccountContent::AsContentMut`, rather then `&mut Self` and will probably had to loose the
    // default implementation for `initialize_bytes()`.  It can be provided as a free function.
    //
    // This would also remove the `Self: Sized` constraint.

    /// Given a reference to bytes of an account that is not initialized, initializes it, according
    /// to the state specified by `state`.  Not initialized means that `Self::is_initialized(data)`
    /// is `false`.
    ///
    /// This method is responsible for writing the account discriminator, as well as the account
    /// content bytes.
    ///
    /// TODO Also, see "Versioning" section in the [`AccountContent`] documentation.
    ///
    /// [`AccountContent`]: crate::account::account_content::AccountContent
    ///
    /// After initialization, `is_initialized(data)` must be `true`.
    ///
    /// This method is responsible for checking that `data` has enough space for writing the
    /// discriminator and the content bytes in there.  `ProgramError::AccountDataTooSmall` should be
    /// returned if this is not the case.
    ///
    /// Default implementation will write `Self::DISCRIMINATOR` as the first
    /// `Self::DISCRIMINATOR.len()` bytes, reinterpret the next `size_of::<Self>()` bytes as `&mut
    /// MaybeUninit<Self>`, passing this reference to the `initialize()` method.
    ///
    /// Unless your type has a non-trivial discriminator layout, it is better to use the default
    /// implementation of this method.
    ///
    /// # Safety
    ///
    /// Caller can only call this if `is_initialized(data)` is `false`.
    ///
    /// This method can only be called if `data` is a reference to zero initialized bytes.
    ///
    /// `initialize_bytes()` implementation must not update any of the bytes into an uninitialized
    /// state.  In other words, it can change bytes into initialized state without reading them, but
    /// it should not assign `MaybeUninit::uninit()` into `data`.
    ///
    /// As `data` is zero initialized, the implementation can take advantage of that - if some of
    /// the bytes need to be zero initialized, they can be be assumed to be already initialized.
    unsafe fn initialize_bytes(
        state: Self::Init<'_>,
        data: &mut [MaybeUninit<u8>],
    ) -> Result<(), ProgramError>;
    //- #[inline]
    //- unsafe fn initialize_bytes(
    //-     state: Self::Init,
    //-     data: &mut [MaybeUninit<u8>],
    //- ) -> Result<(), ProgramError>
    //- where
    //-     Self: Sized,
    //- {
    //-     let discriminator_len = Self::DISCRIMINATOR.len();

    //-     if data.len() < discriminator_len + size_of::<Self>() {
    //-         return Err(ProgramError::AccountDataTooSmall);
    //-     }

    //-     // SAFETY: We checked that the target array contains enough space to write the
    //-     // discriminator.  As we are not reading from the `data` we are not violating the
    //-     // `as_mut_ptr()` safety conditions.
    //-     unsafe {
    //-         data[0]
    //-             .as_mut_ptr()
    //-             .copy_from_nonoverlapping(Self::DISCRIMINATOR.as_ptr(), discriminator_len)
    //-     };

    //-     // SAFETY: As we convert returned pointer into `*mut MaybeUninit<Self>` we are not reading
    //-     // the uninitialized bytes.  And `initialize()` is not expected to read them until they are
    //-     // actually initialized.
    //-     let uninitialized_content: &mut MaybeUninit<Self> =
    //-         unsafe { &mut *data.as_mut_ptr().add(discriminator_len).cast() };
    //-     // SAFETY: `uninitialized_content` was created as a reference to uninitialized bytes.
    //-     let _ = unsafe { Self::initialize(state, uninitialized_content)? };

    //-     Ok(discriminator_len + size_of::<Self>())
    //- }

    //- /// Initializes bytes of the account after the discriminator.
    //- ///
    //- /// The most natural implementation of this method is just a [`MaybeUninit::write()`] call.
    //- /// This would produce an implementation with no unsafe code that is also efficient.
    //- ///
    //- /// Try to avoid `unsafe` code if possible.  But if necessary,
    //- /// [`MaybeUninit::assume_init_mut()`] will provide the desired result.
    //- ///
    //- /// Return value is actually ignore by the caller, but it helps to make sure that the
    //- /// implementation of this method would actually produce a valid value of `Self` in the end.
    //- ///
    //- /// # Safety
    //- ///
    //- /// This method can only be called if `data` is a reference to zero initialized bytes.
    //- ///
    //- /// Implementation must not assign uninitialized values back into `data`.
    //- ///
    //- /// As `data` is zero initialized, the implementation can take advantage of that - if some of
    //- /// the bytes need to be zero initialized, they can be be assumed to be already initialized.
    //- unsafe fn initialize<'account>(
    //-     state: Self::Init,
    //-     data: &'account mut MaybeUninit<Self>,
    //- ) -> Result<&'account mut Self, ProgramError>
    //- where
    //-     Self: Sized;
}

/// This is a helper trait, that helps provide default `Initializable` implementation for
/// `AccountContent` types that use `&mut Self` as `AsContentMut` and `Self` is sized.
///
/// It is automatically implemented for all suitable mutable reference types.  You should not be
/// implementing this trait.
///
/// TODO It is possible to prohibit implementations of this trait outside of this module by adding a
/// local trait into the inheritance chain, but I'm not sure if it is really necessary.
///
/// I wonder if in a more complex situation, there might be a use case for a custom implementation
/// of `ContentIsSizedSelf`?
///
/// ```rust,ignore
/// trait Sealed {}
///
/// #[allow(private_bounds)]
/// pub trait ContentIsSizedSelf: Sealed { /* ... */ }
/// ```
pub trait ContentIsSizedSelf<'account>: Sized {
    type SizedSelf;
}

impl<'account, T: Sized> ContentIsSizedSelf<'account> for &'account mut T {
    type SizedSelf = T;
}

/// A helper function, that converts `data` from `[MaybeUninit<u8>]` into `MaybeUninit<T>` and then
/// invokes a callback to initialize it.
///
/// A somewhat complex type of the `data` argument in the callback is due to an additional
/// flexibility, that allows `AccountContent` types to define what they look like when accessed via
/// a mutable reference.  Rather than using `MaybeUninit<T>` we want to use `T::AsContentMut`.
/// But as `MaybeUninit<U>` needs the value type, rather than a reference to it, we use the
/// [`ContentIsSizedSelf`] helper trait to convert `&mut U` into `U`.
///
/// Callback return type of `T::AsContentMut<'a>` is a small safety measure, that forces the
/// callback code to produce a value reference to a mutable instance of `T` (modulo ability to
/// modify what a mutable reference to `T` looks like).
///
/// Reference returned by the callback is not used, but it guides the callback implementation.
///
/// # Safety
///
/// `T::AsContentMut` must have the same alignment as `data`, which in practice, ideally should be
/// 1.
#[inline(always)]
pub unsafe fn initialize_bytes_with_sized_self<'init, 'account, T, InitWithSelf>(
    state: T::Init<'init>,
    data: &'account mut [MaybeUninit<u8>],
    init_with_self: InitWithSelf,
) -> Result<(), ProgramError>
where
    T: Initializable + AccountContent + 'account,
    T::AsContentMut<'account>: ContentIsSizedSelf<'account>,
    InitWithSelf: for<'a> FnOnce(
        /* state: */ T::Init<'init>,
        /* data: */
        &'a mut MaybeUninit<<T::AsContentMut<'a> as ContentIsSizedSelf<'a>>::SizedSelf>,
    ) -> Result<T::AsContentMut<'a>, ProgramError>,
{
    let discriminator_len = T::DISCRIMINATOR.len();

    if data.len() < discriminator_len + size_of::<T::AsContentMut<'account>>() {
        return Err(ProgramError::AccountDataTooSmall);
    }

    // SAFETY: We checked that the target array contains enough space to write the
    // discriminator.  As we are not reading from the `data` we are not violating the
    // `as_mut_ptr()` safety conditions.
    unsafe {
        data[0]
            .as_mut_ptr()
            .copy_from_nonoverlapping(T::DISCRIMINATOR.as_ptr(), discriminator_len)
    };

    // SAFETY: As we convert returned pointer into `*mut MaybeUninit<Self>` we are not reading
    // the uninitialized bytes.  And `initialize()` is not expected to read them until they are
    // actually initialized.
    let uninitialized_content: &mut MaybeUninit<
        <T::AsContentMut<'account> as ContentIsSizedSelf<'account>>::SizedSelf,
    > = unsafe { &mut *data.as_mut_ptr().add(discriminator_len).cast() };
    // SAFETY: `uninitialized_content` was created as a reference to uninitialized bytes.
    let _ = init_with_self(state, uninitialized_content)?;

    Ok(())
}
