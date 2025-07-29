//! Traits and related machinery in this module help express data sharing between account
//! references.
//!
//! There is a subtle issue related to account objects referencing shared data.  While Rust
//! lifetimes can capture some of the complexities, they do not offer a full range of tools to
//! describe the necessary details.
//!
//! In particular, it would be nice to have a limited form of subtyping applied to the account
//! types, which would allow existence of a type that represents the account data without the
//! content.  Currently these types are called `AnyAccount`, as any account will have these fields.
//!
//! Rust does not provide for a direct way to express a subtyping relationship for types, except
//! indirectly via traits.  But the current solution used for account types does not use traits.
//! While one may start imagining a system that does use traits for expressing subtyping
//! relationship with regard to data sharing between accounts, it may make it harder for the
//! entrypoint macro to write efficient parsers.  I have not explored this enough to be certain.
//!
//! Another consideration is that it is likely that user would want to provide their own account
//! types.  Here I mean types that implement [`ProgramAccountParser`], rather than types that
//! implement [`AccountContent`].  Ideally, we want these new account types to participate in the
//! data sharing using exactly the same rules as any of the existing account types provided by the
//! library.
//!
//! A reasonable property to consider is that if account type `A1` can reference data owned by
//! account type `A2`, then the reserve should also be true.  Meaning that the "can reference data
//! owned by another type" property is symmetric.
//!
//! It is also transitive: if `A1` can reference data owned by `A2`, but `A2` can reference data
//! owned by `A3` it seems reasonable to assume that `A1` should be able to reference data owned by
//! `A3`.  While it is possible to implement types that would adjust account header layout when
//! taking possession of an account data in a way that would break transitivity, it would probably
//! be very counter-intuitive.  As the data sharing is already complex, transitivity becomes a
//! requirement.
//!
//! With both symmetry and transitivity, "can reference data owned by another type" property can
//! also be viewed as defining equivalence classes on the set of all account types.  But, simpler,
//! it just groups all account types into isolated subsets.  All account types in the same subset
//! can share data in-between them.  But there is no sharing across subsets.  We can also call this
//! equivalence a "data sharing subset".
//!
//! This approach has a somewhat unfortunate consequence that would need need multiple different
//! `AnyAccount` types.
//!
//! For example, for read only access that is checked at compile time, we are going to say that
//! `&Account<T>` and `&AnyAccount` are in the same data sharing subset - an account referenced by
//! `&Account<T>` can be also referenced by `&AnyAccount`.  `&OwnAccount` is a special account type
//! that forces ownership checks before data can be accessed, but it falls into the same data
//! sharing subset.
//!
//! Accounts that have their borrowing rules checked at run time use `&SharedAccountRef`, and
//! `&SharedAnyAccountRef` is the `AnyAccount` counterpart here.
//!
//! When account is stated to have a compile time exclusive access, then it is in a sharing subset
//! of it's own.  `&mut Account` can not share data with no other accounts.  Even another instance
//! of `&mut Account`.
//!
//! In order to encode the rules above, account mutual data sharing is expressed as a constant
//! value, indexed by both accounts in question.  An implementation of trait [`CanReference<To>`]
//! for some type `From` is an indication if account type `From` can use data owned by `To`.
//!
//! As the "data sharing subset" is a symmetric property, and trait implementation is not, values
//! for `<From as CanReference<To>>` and `<To as CanReference<From>>` are combined in a symmetrical
//! manager, to define the actual relationship between `To` and `From`.  Transitivity comes from
//! this implementation automatically as well.
//!
//! One additional wrinkle comes from the fact that account types are not defined by the same
//! author.  In particular a number of types are provided by the library and users of the library
//! would not be able to define trait implementations for library provided types.  Yet, it would be
//! good to give them an ability to add new account types to existing data sharing subsets.
//!
//! Rather than defining ability to share data as a flag, `CanReference` defines it as an `i8`
//! value.  To decide on the final value of the sharing ability between `To` and `From` their
//! sharing values are added.  And if the result is above zero, sharing is allowed.
//!
//! By default, if not specified, sharing is set to have a value of `0`.  So that account types that
//! were not designed to work with each other, sharing is prohibited.  This works for relatively
//! shallow subtyping chains, which should be good enough for this problem.

/// Defines sharing relationship between two account types.
///
/// Implementation of this trait for account `From` indicates if it can share data with the account
/// `To` if
/// ```rust,ignore
///   <From as CanReference<To>>::can_reference() + <To as CanReference<From>>::can_reference()
/// ```
/// is positive.  Note that if `CanReference` is not implemented for a particular direction between
/// two account types, it is equivalent to an implementation that returns `0`.
///
/// See [`crate::entrypoint_v2::can_reference`] module documentation for details.
///
// TODO It would be great to mark this trait as `const`, to be able to make sure that
// the score calculation happens at compile time and produces the most efficient code
// possible.
//
// That would be only possible when RFC 3762 is stabilized.
// https://github.com/rust-lang/rfcs/pull/3762
// https://github.com/rust-lang/rust/issues/143874
//
// TODO Not sure how to make sure that this transition would be possible.  Even if I mark this trait
// us `unsafe` with a requirement that all implementations only use `const` expressions, it still
// would be a breaking change to change the trait type to `const triat`, right?
//
// TODO One other option is to use `typenum`.  It is not the first place I could use it, so maybe
// this is something Pinocchio should consider as a dependency.  It will certainly simplify compile
// time computations and optimizations.
pub trait CanReference<To> {
    fn can_reference() -> i8;
}

/// Implements [`CanReference`] between two types with the specified reference score.
/// A single account type may need to implement `CanReference` multiple times, if it belongs to a
/// large enough data sharing subset.  So this macro removes some of the visual noise.
///
/// See [`crate::entrypoint_v2::can_reference`] module documentation for details.
#[macro_export]
macro_rules! impl_CanReference {
    (< $( $generic_args:tt ),* > $from:ty => $to:ty : $score:literal) => {
        impl<$( $generic_args ),*> CanReference<$to> for $from {
            fn can_reference() -> i8 {
                $score
            }
        }
    };

    ($from:ty => $to:ty : $score:literal) => {
        impl CanReference<$to> for $from {
            fn can_reference() -> i8 {
                $score
            }
        }
    };
}

use core::marker::PhantomData;

// This is a little hack that allows the rest of the code in `pinocchio` to use
// `impl_CanReference!()` as if it is defined in `entrypoint_v2::can_reference`.
pub use crate::impl_CanReference;

/// Helper that is used to simulate specialization for `CanReference` to provide a default score of
/// `0`.
///
/// See
/// https://github.com/dtolnay/case-studies/tree/master/autoref-specialization#autoref-based-stable-specialization
/// for a detailed explanation of the used approach.
// TODO Should this type be `#[doc(hidden)]`?
#[derive(Debug)]
pub struct ComputeCanReference<From, To> {
    _from: PhantomData<From>,
    _to: PhantomData<To>,
}

impl<From, To> ComputeCanReference<From, To> {
    #[inline(always)]
    pub const fn dispatcher() -> Self {
        Self {
            _from: PhantomData,
            _to: PhantomData,
        }
    }
}

/// Helper that is used to compute `CanReference` score while simulating specialization, with a
/// default score of `0` when `CanReference` is not implemented fro a particular pair of `From` and
/// `To`.
// TODO Should this trait be `#[doc(hidden)]`?
// TODO This should be `const trait`, when RFC 3762 is stabilized.
// https://github.com/rust-lang/rfcs/pull/3762
// https://github.com/rust-lang/rust/issues/143874
pub trait CanReferenceScore<From, To> {
    fn score(self) -> i8;
}

impl<From: CanReference<To>, To> CanReferenceScore<From, To> for ComputeCanReference<From, To> {
    #[inline(always)]
    fn score(self) -> i8 {
        <From as CanReference<To>>::can_reference()
    }
}

impl<From, To> CanReferenceScore<From, To> for &ComputeCanReference<From, To> {
    #[inline(always)]
    fn score(self) -> i8 {
        0
    }
}
