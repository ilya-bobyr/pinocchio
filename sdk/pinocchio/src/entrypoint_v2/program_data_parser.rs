//! When programs are invoked, all the input is provided as a byte buffer.  `AccountDataParser` is a
//! trait that helps describe a parsing process for this data.

use core::mem::size_of;

use crate::{program_error::ProgramError, pubkey::Pubkey};

/// A trait that describes a type that can be parsed from a program input buffer.
///
/// At a very high level, we try to be as efficient as possible.  Consider that the program input
/// has a static lifetime.  And so parsed version can easily maintain references to the input
/// data.
///
/// Introduction of the `'data` lifetime at the trait level (rather then at the [`parse()`] method
/// level) allows the implementers of this trait to reference the lifetime when implementing
/// references to data structures.  For example, it would not be possible to correctly type the
/// following implementation without `'data` being at the trait level:
///
/// ```rust,ignore
/// impl<'data> ProgramDataParser<'data> for &'data str {
///     /* ... */
///     fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error> {
///         /* ... */
///     }
/// }
/// ```
pub trait ProgramDataParser<'data> {
    /// Parsing error that can be produced when parsing this particular data type.
    ///
    /// As [`parse()`] is used by the [`entrypoint!()`] macro, produced error must be convertible
    /// into [`ProgramError`] automatically.  Otherwise parser written by the macro would not be
    /// able to propagate them.  Thus, the `Into<ProgramError>` constraint is required.
    type Error: Into<ProgramError>;

    /// Converts a prefix of `data` into an instance of `Self`, potentially, with references to
    /// `'data`, and returns a trailing portion of `data`.  Or returns an error if `data` prefix can
    /// not be treated as an instance of `Self`.
    fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error>
    where
        Self: Sized + 'data;
}

impl<'data> ProgramDataParser<'data> for u8 {
    type Error = ProgramError;

    fn parse(data: &'data [u8]) -> Result<(u8, &'data [u8]), Self::Error> {
        let Some((&value, data)) = data.split_first() else {
            return Err(ProgramError::InvalidInstructionData);
        };

        Ok((value, data))
    }
}

impl<'data> ProgramDataParser<'data> for i8 {
    type Error = ProgramError;

    fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error>
    where
        Self: Sized + 'data,
    {
        let Some((&value, data)) = data.split_first() else {
            return Err(ProgramError::InvalidInstructionData);
        };

        Ok((value as i8, data))
    }
}

macro_rules! impl_ProgramDataParser_with_from_le_bytes {
    ($type:ident) => {
        impl<'data> ProgramDataParser<'data> for $type {
            type Error = ProgramError;

            fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error>
            where
                Self: Sized + 'data,
            {
                let Some((value, data)) = data.split_first_chunk::<{ size_of::<$type>() }>() else {
                    return Err(ProgramError::InvalidInstructionData);
                };

                let value = <$type>::from_le_bytes(*value);
                Ok((value, data))
            }
        }
    };
}

impl_ProgramDataParser_with_from_le_bytes!(u16);
impl_ProgramDataParser_with_from_le_bytes!(u32);
impl_ProgramDataParser_with_from_le_bytes!(u64);

impl_ProgramDataParser_with_from_le_bytes!(i16);
impl_ProgramDataParser_with_from_le_bytes!(i32);
impl_ProgramDataParser_with_from_le_bytes!(i64);

impl<'data> ProgramDataParser<'data> for &'data Pubkey {
    type Error = ProgramError;

    fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error> {
        let Some((pubkey_bytes, data)) = data.split_first_chunk::<32>() else {
            return Err(ProgramError::InvalidInstructionData);
        };

        Ok((pubkey_bytes.into(), data))
    }
}

// TODO This is a rather arbitrary implementation.  It assumes that the string length is encoded in
// a single byte preceding the string bytes.  Is this really the most common encoding, such that we
// want to use it by default?  If not, we may want to provide this implementation via a wrapper
// type.
impl<'data> ProgramDataParser<'data> for &'data str {
    type Error = ProgramError;

    fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error> {
        let Some((&len, data)) = data.split_first() else {
            return Err(ProgramError::InvalidInstructionData);
        };

        // TODO When MSRV goes to 1.80.0 use `slice::split_at_checked()`:
        //
        // ```rust
        // let Some((str_bytes, data)) = data.split_at_checked(usize::from(len)) else {
        //     return Err(ProgramError::InvalidInstructionData);
        // };
        // ```
        if data.len() < len.into() {
            return Err(ProgramError::InvalidInstructionData);
        }
        // SAFETY: Check above verifies `len` is within `data`.
        let (str_bytes, data) = unsafe { data.split_at_unchecked(usize::from(len)) };

        let Ok(str_utf8) = ::core::str::from_utf8(str_bytes) else {
            return Err(ProgramError::InvalidInstructionData);
        };

        Ok((str_utf8, data))
    }
}

/// This is a special implementation that just captures all the rest of the provided program data.
/// Allows one to describe arbitrary complex parsing outside of the provided parsing framework.
impl<'data> ProgramDataParser<'data> for &'data [u8] {
    type Error = ProgramError;

    fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error> {
        Ok((data, &[]))
    }
}

/// Parses a single byte that indicates if the value is present or not.
/// `0` means `None`, `1` means that subsequent bytes need to be parsed as a value of `T`.
/// Any other value of the first byte produces an error.
impl<'data, T> ProgramDataParser<'data> for Option<T>
where
    T: ProgramDataParser<'data> + 'data,
    T::Error: From<ProgramError>,
{
    type Error = T::Error;

    fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error> {
        let Some((has_value, data)) = data.split_first() else {
            return Err(ProgramError::InvalidInstructionData.into());
        };

        match has_value {
            0 => Ok((None, data)),
            1 => T::parse(data).map(|(res, data)| (Some(res), data)),
            _ => Err(ProgramError::InvalidInstructionData.into()),
        }
    }
}
