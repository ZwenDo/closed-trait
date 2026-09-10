#![doc = include_str!("../README.md")]
#![no_std]

pub use closed_trait_macros::{enumerate, if_implements_fn, sealed};

/// A type that can be turned into the enum of its sealed trait.
///
/// [`enumerate`] implements this for every permitted type, and makes `Enumerable<TheSealedTrait>` a
/// supertrait of the sealed trait. Naming the enum in the bound is what lets a caller reach it
/// through the trait alone:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
///
/// struct Square;
///
/// #[enumerate]
/// #[sealed(Square)]
/// trait Shape {}
///
/// impl Shape for Square {}
///
/// // No import: the supertrait bound carries `into_enum` in with `S: Shape`.
/// fn describe<S: Shape>(s: S) {
///     match s.into_enum() {
///         AnyShape::Square(s) => { /* .. */ }
///     }
/// }
/// # fn main() { describe(Square); }
/// ```
///
/// Note that `Enumerable` did not have to be imported above: the supertrait bound brings
/// `into_enum` into scope through `S: TheSealedTrait`. Calling it on a concrete type rather than a
/// generic one does need the import.
///
/// `From` is implemented alongside it in the other direction, so `From::from` and `Into::into` work
/// too.
pub trait Enumerable<Enum> {
    /// Wraps `self` in the variant of `Enum` that holds this type.
    fn into_enum(self) -> Enum;
}

/// A type that can lend itself to the *borrowing* enum of its sealed trait.
///
/// [`enumerate`] implements this for every permitted type and makes
/// `for<'a> EnumerableRef<'a, TheSealedTraitRef<'a>>` a supertrait. The lifetime is a parameter of
/// the trait rather than of the method, so the higher-ranked bound is nameable in the supertrait
/// list, which is what lets a caller reach the enum from a plain `&S`:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # use closed_trait::EnumerableRef;
///
/// struct Square;
///
/// #[enumerate]
/// #[sealed(Square)]
/// trait Shape {}
///
/// impl Shape for Square {}
///
/// # fn main() {
/// let s = &Square;
/// match s.as_enum_ref() {
///     AnyShapeRef::Square(s) => { /* .. */ }
/// }
/// # }
/// ```
///
/// [`Enumerable`] cannot do this: `into_enum` takes `self`, so reaching the owned enum means owning
/// the value. The borrowing enum is also the cheaper one to pass, being a pointer and a discriminant
/// rather than as large as the biggest permitted type.
pub trait EnumerableRef<'a, EnumRef> {
    /// Wraps `&self` in the variant of `EnumRef` that holds this type.
    fn as_enum_ref(&'a self) -> EnumRef;
}

/// A type that can lend itself *mutably* to the borrowing enum of its sealed trait.
///
/// The counterpart of [`EnumerableRef`], reached from a `&mut S` the same way:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # use closed_trait::EnumerableMut;
///
/// struct Square;
///
/// #[enumerate]
/// #[sealed(Square)]
/// trait Shape {}
///
/// impl Shape for Square {}
///
/// # fn main() {
/// let mut s = &mut Square;
/// match s.as_enum_mut() {
///     AnyShapeMut::Square(s) => { /* .. */ }
/// }
/// # }
/// ```
///
/// Unlike the shared enum this one is neither `Clone` nor `Copy`, a unique reference being neither.
pub trait EnumerableMut<'a, EnumMut> {
    /// Wraps `&mut self` in the variant of `EnumMut` that holds this type.
    fn as_enum_mut(&'a mut self) -> EnumMut;
}
