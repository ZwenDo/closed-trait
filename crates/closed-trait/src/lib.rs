#![doc = include_str!("../README.md")]
#![no_std]

pub use closed_trait_macros::{enumerate, sealed};

/// A type that can be turned into the enum of its sealed trait.
///
/// [`enumerate`] implements this for every permitted type, and makes
/// `Enumerable<AnyShape>` a supertrait of the sealed trait. Naming the enum in
/// the bound is what lets a caller reach it through the trait alone:
///
/// ```
/// use closed_trait::{enumerate, sealed};
///
/// pub struct Square { pub side: i32 }
/// pub struct Circle { pub radius: i32 }
///
/// #[enumerate]
/// #[sealed(Square, Circle)]
/// pub trait Shape {}
///
/// impl Shape for Square {}
/// impl Shape for Circle {}
///
/// /// Generic over the trait, yet able to match exhaustively.
/// fn corners<S: Shape>(shape: S) -> u32 {
///     match shape.into_enum() {
///         AnyShape::Square(_) => 4,
///         AnyShape::Circle(_) => 0,
///     }
/// }
///
/// fn main() {
///     assert_eq!(corners(Square { side: 1 }), 4);
///     assert_eq!(corners(Circle { radius: 1 }), 0);
/// }
/// ```
///
/// Note that `Enumerable` did not have to be imported above: the supertrait
/// bound brings `into_enum` into scope through `S: Shape`. Calling it on a
/// concrete type rather than a generic one does need the import.
///
/// The enum is a type *parameter* rather than an associated type so that one
/// type can belong to several sealed traits at once, since an associated type could
/// only be chosen once per implementor. The cost is that `into_enum` on a
/// concrete type belonging to more than one needs the target spelled out, by
/// annotation or turbofish. `From` sidesteps that, since the enum is named by
/// the conversion itself.
///
/// `From` is implemented alongside it in the other direction, so
/// `AnyShape::from(square)` and `square.into()` work too.
pub trait Enumerable<Enum> {
    /// Wraps `self` in the variant of `Enum` that holds this type.
    fn into_enum(self) -> Enum;
}

/// A type that can lend itself to the *borrowing* enum of its sealed trait.
///
/// [`enumerate`] implements this for every permitted type and makes
/// `for<'a> EnumerableRef<'a, AnyShapeRef<'a>>` a supertrait. The
/// lifetime is a parameter of the trait rather than of the method, so the
/// higher-ranked bound is nameable in the supertrait list, which is what lets
/// a caller reach the enum from a plain `&S`:
///
/// ```
/// use closed_trait::{enumerate, sealed};
///
/// pub struct Square { pub side: i32 }
/// pub struct Circle { pub radius: i32 }
///
/// #[enumerate(match_any)]
/// #[sealed(Square, Circle)]
/// pub trait Shape {}
///
/// impl Shape for Square {}
/// impl Shape for Circle {}
///
/// /// Takes a reference, yet still matches exhaustively.
/// fn corners<S: Shape>(shape: &S) -> u32 {
///     match shape.as_enum_ref() {
///         AnyShapeRef::Square(_) => 4,
///         AnyShapeRef::Circle(_) => 0,
///     }
/// }
///
/// fn main() {
///     assert_eq!(corners(&Square { side: 1 }), 4);
///     assert_eq!(corners(&Circle { radius: 1 }), 0);
/// }
/// ```
///
/// [`Enumerable`] cannot do this: `into_enum` takes `self`, so reaching the
/// owned enum means owning the value. The borrowing enum is also the cheaper
/// one to pass, being a pointer and a discriminant rather than as large as the
/// biggest permitted type.
pub trait EnumerableRef<'a, EnumRef> {
    /// Wraps `&self` in the variant of `EnumRef` that holds this type.
    fn as_enum_ref(&'a self) -> EnumRef;
}

/// A type that can lend itself *mutably* to the borrowing enum of its sealed
/// trait.
///
/// The counterpart of [`EnumerableRef`], and reached from a `&mut S` the same
/// way:
///
/// ```
/// use closed_trait::{enumerate, sealed};
///
/// pub struct Square { pub side: i32 }
/// pub struct Circle { pub radius: i32 }
///
/// #[enumerate]
/// #[sealed(Square, Circle)]
/// pub trait Shape {
///     fn grow(&mut self);
/// }
///
/// impl Shape for Square { fn grow(&mut self) { self.side += 1; } }
/// impl Shape for Circle { fn grow(&mut self) { self.radius += 1; } }
///
/// fn grow_twice<S: Shape>(shape: &mut S) {
///     match shape.as_enum_mut() {
///         AnyShapeMut::Square(s) => { s.grow(); s.grow(); }
///         AnyShapeMut::Circle(c) => { c.grow(); c.grow(); }
///     }
/// }
///
/// fn main() {
///     let mut square = Square { side: 1 };
///     grow_twice(&mut square);
///     assert_eq!(square.side, 3);
/// }
/// ```
///
/// Unlike the shared enum this one is neither `Clone` nor `Copy`, a unique
/// reference being neither.
pub trait EnumerableMut<'a, EnumMut> {
    /// Wraps `&mut self` in the variant of `EnumMut` that holds this type.
    fn as_enum_mut(&'a mut self) -> EnumMut;
}
