//! Attribute macros for the [`closed-trait`] crate. Use them through that crate,
//! which re-exports both and provides the items the generated code refers to.
//!
//! [`closed-trait`]: https://docs.rs/closed-trait
mod enumerate;
mod implements;
mod sealed;
mod util;

use proc_macro::TokenStream;

/// Seals a trait so that only the listed types can implement it, and every listed type must
/// implement it.
///
/// ```compile_fail
/// # use closed_trait::sealed;
/// #[sealed(Circle, Square)] // error: Square does not implement Shape
/// trait Shape {}
///
/// struct Circle;
/// impl Shape for Circle {}
///
/// struct Square;
///
/// impl Shape for i32 {} // error: i32 is not permitted
/// # fn main() {}
/// ```
///
/// # Entries
///
/// An entry is a type, written plainly or as a path, as in `#[sealed(Square, shapes::Circle)]`, and
/// that is all of it where neither the trait nor the type is generic. Cases where one or both of
/// them are generic are presented in the next sections.
///
/// ## A generic trait
///
/// A generic trait has to be told which of its instantiations the entry implements.
/// `Entry: Trait<..>` says which:
///
/// ```
/// # use closed_trait::sealed;
/// struct Plain;
///
/// #[sealed(Plain: Store<i32>)]
/// trait Store<T> {}
///
/// impl Store<i32> for Plain {}
/// # fn main() {}
/// ```
///
/// One instantiation can be enough, but the type may implement the trait at every one of them.
/// `for<..>` declares a parameter for the entry to instantiate with:
///
/// ```
/// # use closed_trait::sealed;
/// struct Plain;
///
/// // every `Store<T>`, not just one
/// #[sealed(for<T> Plain: Store<T>)]
/// trait Store<T> {}
///
/// impl<T> Store<T> for Plain {}
/// # fn main() {}
/// ```
///
/// A parameter the binder declares can carry bounds:
///
/// ```
/// # use closed_trait::sealed;
/// # use std::fmt::Debug;
/// struct Plain;
///
/// #[sealed(for<T: Debug> Plain: Store<T>)]
/// trait Store<T> {}
///
/// impl<T: Debug> Store<T> for Plain {}
/// # fn main() {}
/// ```
///
/// The bounds are part of what is sealed: `Plain` is permitted `Store<T>` only where `T: Debug`, so
/// an `impl<T> Store<T> for Plain` covering every `T` is refused.
///
/// Lifetimes, types and const parameters can be declared together, lifetimes first (as in
/// `for<'a, T: Clone, const N: usize>`), and each is written exactly as it would be on an `impl`.
///
/// ## A generic type
///
/// The parameters a `for<..>` declares serve the type just as well, which is how a trait with no
/// parameters of its own seals a generic type:
///
/// ```
/// # use closed_trait::sealed;
/// # struct Square;
/// # impl Shape for Square {}
/// # struct Circle;
/// # impl Shape for Circle {}
/// struct Ref<'a, T>(&'a T);
///
/// #[sealed(
///     for<'a, T: Shape> Ref<'a, T>,
///     Square,
///     Circle,
/// )]
/// trait Shape {}
///
/// impl<'a, T: Shape> Shape for Ref<'a, T> {}
/// # fn main() {}
/// ```
///
/// ## A generic type under a generic trait
///
/// One binder covers both, and a name it declares may stand in the type and the instantiation
/// alike.
///
/// ```
/// # use closed_trait::sealed;
/// struct Boxed<U>(U);
///
/// // every `Boxed<U>`, each at the matching `Store<U>`
/// #[sealed(for<U> Boxed<U>: Store<U>)]
/// trait Store<T> {}
///
/// impl<U> Store<U> for Boxed<U> {}
/// # fn main() {}
/// ```
///
/// The binder's names are its own, so where they go is what counts, not what they are called:
/// `for<U, V> Pair<U, V>: Store<V, U>` seals `Pair<U, V>` at `Store<V, U>`, swapped. And the
/// instantiation's arguments are ordinary types, so a parameter can sit inside a larger one rather
/// than be the argument itself: `for<T> Keyed<T>: Store<Vec<T>>`.
///
/// ## `as Name`
///
/// Names the entry. Only [`enumerate`][macro@enumerate] reads it: a variant is otherwise named
/// after the type it holds, and two entries whose names come out the same would be one variant
/// twice, which is refused. `as Name` gives one of them a name of its own. This happens in two
/// ways.
///
/// **Different types whose last segment matches.** Here the name settles which is which:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// mod a { pub struct Foo; }
/// mod b { pub struct Foo; }
///
/// #[enumerate]
/// // without `as`, both would take the last segment `Foo`
/// #[sealed(a::Foo as Left, b::Foo as Right)]
/// trait Shape {}
///
/// impl Shape for a::Foo {}
/// impl Shape for b::Foo {}
///
/// # fn main() {
/// let _ = AnyShape::Left(a::Foo); // see enumerate
/// # }
/// ```
///
/// **The same type listed twice**, which is how one type reaches the enum at more than one
/// instantiation. There the name is not a nicety but required, since both entries would otherwise
/// be the `Plain` variant:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # use closed_trait::Enumerable;
/// struct Plain;
/// struct Boxed<T>(pub T);
///
/// #[enumerate]
/// #[sealed(Plain: Store<i32>, Plain as PlainF64: Store<f64>, for<T> Boxed<T>: Store<T>)]
/// trait Store<T> {}
///
/// impl Store<i32> for Plain {}
/// impl Store<f64> for Plain {}
/// impl<T> Store<T> for Boxed<T> {}
///
/// # fn main() {
/// // the one type reaching two different enum instantiations
/// assert!(matches!(Plain.into_enum(), AnyStore::<i32>::Plain(_)));
/// assert!(matches!(Plain.into_enum(), AnyStore::<f64>::PlainF64(_)));
/// # }
/// ```
///
/// The name settles the *variant* only. The two entries must also pin different arguments, and
/// some entry (`Boxed<T>` here) has to *mention* `T`. The enum is generic over the parameters its
/// variants use, not over the trait's, since an enum may not declare one no variant uses. Drop
/// `Boxed<T>` and nothing is left to be generic over: both entries become variants of one plain
/// `AnyStore`, so `Plain` converts into it two ways and `into_enum` has two answers. `enumerate`
/// refuses that. Keeping the enum generic is what puts the two entries in `AnyStore<i32>` and
/// `AnyStore<f64>`, one `Plain` apiece.
///
/// ## All of it at once
///
/// A binder, the type, a name and the instantiation it implements, in that order:
///
/// ```
/// # use closed_trait::sealed;
/// struct Ref<'a, T>(&'a T);
///
/// #[sealed(
///     for<'a, T> Ref<'a, T> as RefStore: Store<i32>
/// )]
/// trait Store<T> {}
///
/// impl<'a, T> Store<i32> for Ref<'a, T> {}
/// # fn main() {}
/// ```
///
/// # The list is checked in both directions
///
/// Every entry is checked, which is why a trait that declares type or const parameters needs them
/// supplied for each of its entries, and the instantiation is what supplies them:
/// `Plain: Store<i32>` pins them, `for<T> Boxed<T>: Store<T>` passes on what its binder declared.
/// An entry without one is refused, since nothing could then tell whether it implements the trait
/// at all. A trait declaring none asks nothing, which is why `#[sealed(Square, Circle)]` above
/// needs no annotation.
///
/// A lifetime is never asked for, and not merely because inference usually copes. A type cannot
/// implement the same trait at two different lifetimes: two such impls overlap, and coherence
/// rejects them, so there is never more than one candidate to disambiguate. `#[sealed(Plain)]`
/// under `trait Foo<'a>` is therefore accepted *and* checked: an entry implementing no `Foo` at all
/// is still caught.
///
/// # The seal is as precise as the list
///
/// The marker carries the same type and const parameters the trait does, so `Plain: Store<i32>`
/// permits `Plain` to implement `Store<i32>` and nothing else: an unlisted `impl Store<f64> for
/// Plain` is rejected:
///
/// ```compile_fail
/// # use closed_trait::sealed;
/// struct Plain;
///
/// #[sealed(Plain: Store<i32>)]
/// trait Store<T> {}
///
/// impl Store<i32> for Plain {}
/// impl Store<f64> for Plain {} // error: not permitted to implement `Store` here
/// # fn main() {}
/// ```
///
/// An entry whose binder supplies them instead, like `for<T> Boxed<T>: Store<T>`, permits every
/// instantiation, which is what the binder says. Lifetimes are not on the marker, for the reason
/// above: they could never tell two entries apart.
///
/// # What the seal is worth
///
/// The marker trait is private to the module the attribute is written in, and carries a supertrait
/// private one level deeper. Naming the marker is therefore not enough to satisfy it: the only
/// place both can be implemented is inside the generated module, which nothing but this macro
/// writes. Code sitting directly beside the sealed trait cannot opt a type in, which a single level
/// of privacy would have allowed.
///
/// The cost is that permitted types must be nameable from that module, so they have to live at
/// module level. **A type declared inside a function body cannot be sealed**, because no module
/// nested in a function can refer to it.
#[proc_macro_attribute]
pub fn sealed(args: TokenStream, item: TokenStream) -> TokenStream {
    sealed::sealed(args, item)
}

/// Generates enums holding the types a trait is sealed to and macros rules to work with these
/// enums.
///
/// Reads its type list from the `#[sealed(..)]` attribute below it, so it must be written **above**;
/// attribute macros run top down, and `#[sealed]` consumes itself when it expands.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// // on a concrete type `into_enum` needs the trait in scope; a generic
/// // `S: Shape` gets it from the supertrait bound
/// use closed_trait::Enumerable;
///
/// struct Square;
/// struct Circle;
///
/// #[enumerate]
/// #[sealed(Square, Circle)]
/// trait Shape {}
///
/// impl Shape for Square {}
/// impl Shape for Circle {}
///
/// # fn main() {
/// let shape: AnyShape = Square.into_enum();
/// match shape {
///     AnyShape::Square(_) => {},
///     AnyShape::Circle(_) => {},
/// # }
/// }
/// ```
///
/// # The three enums
///
/// All three are generated by default, each with one variant per entry named after the type's last
/// path segment, and each taking the trait's visibility. A type not in upper camel case therefore
/// gives a variant that is not either (`i32` yields an `i32` variant), so the enums carry
/// `#[allow(non_camel_case_types)]`: the name came from a type, not from a choice the caller made.
/// `as Name` is there for anyone who would rather write `I32`. Given the base `Shape` sealed
/// trait:
///
/// | enum              | holds            | reached from                               |
/// | ----------------- | ---------------- | ------------------------------------------ |
/// | `AnyShape`        | `Square`         | `into_enum`, or `From`                     |
/// | `AnyShapeRef<'a>` | `&'a Square`     | `as_enum_ref`, `From`, or `owned.as_ref()` |
/// | `AnyShapeMut<'a>` | `&'a mut Square` | `as_enum_mut`, `From`, or `owned.as_mut()` |
///
/// Each brings a supertrait with it: `Enumerable<AnyShape>` and the higher-ranked `for<'a>
/// EnumerableRef<'a, AnyShapeRef<'a>>` and its `Mut` counterpart, which is what makes the enums
/// reachable from a generic `S: Shape` without naming them.
///
/// The borrowing pair is what `into_enum` cannot give you: taking `self`, it needs the value moved
/// in, so a `&S` has no route to the owned enum at all. They are also cheaper to pass, being a
/// pointer and a discriminant rather than as wide as the largest permitted type. The shared one
/// derives `Clone` and `Copy`.
///
/// Conversions *between* the three (`as_ref` and `as_mut` on the owned enum, and `as_ref` on the
/// unique one, which reborrows) come as inherent methods. `no_bridge` leaves them out.
///
/// # Options
///
/// Written bare, an option applies to all three enums. Written inside `owned(..)`, `ref(..)` or
/// `mut(..)` it applies to that one, and a specific option beats a grouped one.
///
/// ## `name = ..`
///
/// Names the enums, which are `Any{Trait}`, `Any{Trait}Ref` and `Any{Trait}Mut` by default.
///
/// Grouped, it is a **base** that each kind extends, so `name = "Shapes"` gives `Shapes`,
/// `ShapesRef` and `ShapesMut`. Specific, it is the name itself: `ref(name = "ShapeView")` gives
/// exactly `ShapeView`.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # struct Square;
///
/// #[enumerate(name = "Shapes", ref(name = "ShapeView"))]
/// #[sealed(Square)]
/// trait Shape {}
///
/// impl Shape for Square {}
///
/// # fn main() {
/// let mut owned: Shapes        = Shapes::Square(Square);
/// let mutable:   ShapesMut<'_> = owned.as_mut();
/// let reference: ShapeView<'_> = mutable.as_ref();
/// # }
/// ```
///
/// ## `no_bridge`
///
/// Prevents generating the conversion methods written *on* an enum. `owned(no_bridge)` drops
/// `as_ref(&self)` and `as_mut(&mut self)`; `mut(no_bridge)` drops the reborrowing `as_ref(&self)`;
/// a bare `no_bridge` drops all three. Nothing is written on the shared enum, so `ref(no_bridge)`
/// is refused rather than silently doing nothing.
///
/// ## `attrs = ".."`
///
/// Attributes to put on a generated enum, verbatim: derives, `#[non_exhaustive]`, `#[repr(..)]`,
/// anything. It is the one option that **must** be specific. What is valid differs between
/// the three: the shared enum already derives `Copy`, and the unique one cannot derive `Clone` at
/// all, so one spelling spread across them would be a trap. A bare `attrs` is an error saying so.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # #[derive(Clone, Debug, PartialEq)] pub struct Square;
///
/// #[enumerate(owned(attrs = "#[derive(Clone, Debug, PartialEq)] #[non_exhaustive]"))]
/// #[sealed(Square)]
/// trait Shape {}
///
/// # impl Shape for Square {}
/// # fn main() {}
/// ```
///
/// Documentation is the exception: the enum's docs are generated and identical for every sealed
/// trait, so a `///` or `#[doc = ".."]` here is an error.
///
/// ## `crate = ".."`
///
/// Where the generated code should look for `Enumerable`. Defaults to `::closed_trait`, which is
/// right whenever `closed-trait` is a direct dependency under its own name.
///
/// It is wrong in two cases, and both fail with `cannot find `closed_trait` in the crate root` even
/// though nothing in the caller's source names it:
///
/// - the dependency was renamed, as in `st = { package = "closed-trait" }`;
/// - the macros are reached through a re-export, so the caller does not depend
///   on `closed-trait` at all.
///
/// Point it at whatever crate re-exports `Enumerable`:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # pub struct Square;
/// #[enumerate(crate = "::closed_trait")]
/// #[sealed(Square)]
/// trait Shape {}
///
/// # impl Shape for Square {}
/// # fn main() {}
/// ```
///
/// ## `match_any`
///
/// Generates `match_any_{trait}!`, a macro that expands to a `match` over every variant. A `match`
/// on the enum already tells the variants apart; what this adds is one body that runs against the
/// *concrete* type, which a closure cannot express because Rust has no generic closures.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
///
/// struct Square { side: i32 }
/// struct Circle { radius: i32 }
///
/// #[enumerate(match_any)]
/// #[sealed(Square, Circle)]
/// trait Shape { fn area(&self) -> i32; }
///
/// # impl Shape for Square { fn area(&self) -> i32 { self.side * self.side } }
/// # impl Shape for Circle { fn area(&self) -> i32 { 3 * self.radius * self.radius } }
///
/// # fn main() {
/// let shape = AnyShape::from(Square { side: 3 });
/// let area = match_any_shape!(shape, s => s.area());
/// assert_eq!(9, area);
/// # }
/// ```
///
/// The value may be given by `&`, by `&mut` or by value; match ergonomics make
/// the binding follow it, so one macro covers all three. The binding has to be
/// named by the caller.
///
/// It is a `match`, not a closure, and the difference is the point:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # pub struct Square { pub side: i32 }
/// # pub struct Circle { pub radius: i32 }
/// # #[enumerate(match_any)]
/// # #[sealed(Square, Circle)]
/// # pub trait Shape { fn area(&self) -> i32; }
/// # impl Shape for Square { fn area(&self) -> i32 { self.side * self.side } }
/// # impl Shape for Circle { fn area(&self) -> i32 { 3 * self.radius * self.radius } }
/// fn first_greater(shapes: &[AnyShape], value: i32) -> Option<i32> {
///     for shape in shapes {
///         // `return` leaves `first_big`, which a method taking the body could
///         // never do
///         match_any_shape!(shape, s => if s.area() > value { return Some(s.area()) });
///     }
///     None
/// }
///
/// # fn main() {}
/// ```
///
/// The body may also move anything it owns, since only one arm ever runs, and it can be `async`.
/// To handle some variants differently, match first and let the last arm fall through:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # pub struct Square { pub side: i32 }
/// # pub struct Circle { pub radius: i32 }
/// # #[enumerate(match_any)]
/// # #[sealed(Square, Circle)]
/// # pub trait Shape { fn area(&self) -> i32; }
/// # impl Shape for Square { fn area(&self) -> i32 { self.side * self.side } }
/// # impl Shape for Circle { fn area(&self) -> i32 { 3 * self.radius * self.radius } }
/// # fn main() {
/// # let shape = AnyShape::from(Square { side: 3 });
/// let cost = match &shape {
///     AnyShape::Square(_) => 0,
///     other => match_any_shape!(other, s => s.area()),
/// };
/// # assert_eq!(cost, 0);
/// # }
/// ```
///
/// Two things follow from it being a macro. The enum and the trait must be in scope where it is
/// called, because a `macro_rules!` body resolves paths at the call site. And the body is *copied*
/// into every arm.
///
/// That copying is worth being deliberate about. It costs compile time in proportion to the number
/// of variants, since the body is type-checked once per arm, and one mistake in it is reported once
/// per arm too. Nesting one of these inside another squares the count.
///
/// The option takes an optional name, so `match_any("match_shape")` generates `match_shape!`
/// instead. Whether the macro can leave the crate depends on the trait's visibility, and so does
/// whether it can collide, see [Visibility](#visibility).
///
/// # Visibility
///
/// Everything generated takes the trait's own visibility: the three enums, the conversions between
/// them, and the names the match macros are reached through. There is no option to change it: the
/// enums appear in the trait's supertrait bounds, so anything narrower would put a private type in
/// a public interface.
///
/// It decides one thing beyond reach, though. A `macro_rules!` cannot leave the crate that defines
/// it without `#[macro_export]`, and that always plants it at the crate root. So a macro generated
/// for a `pub` trait goes there under a hidden name and is aliased beside the enum, while one for
/// any narrower trait simply stays where it was written:
///
/// | the trait is      | the macro is                               | usable from another crate | can collide |
/// | ----------------- | ------------------------------------------ | ------------------------- | ----------- |
/// | `pub`             | at the crate root, aliased beside the enum | yes                       | yes         |
/// | anything narrower | where it was written                       | no                        | no          |
///
/// Colliding means two traits of the same name, in different modules, both asking for `match_any`:
/// their hidden root names would be the same, and one of them needs `match_any("other_name")`.
///
/// # Generics
///
/// A generic trait's enum declares the parameters its variants are generic over, with their bounds,
/// not those of the trait: an enum may not declare a parameter no variant uses. Where every entry
/// pins its arguments, none is left to declare and the enum is plain:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
///
/// // every entry fixes its argument, so `AnyValue` is a plain enum
/// #[enumerate]
/// #[sealed(
///     i32: Value<i32>,
///     f64: Value<f64>
/// )]
/// trait Value<T> {}
///
/// impl Value<i32> for i32 {}
/// impl Value<f64> for f64 {}
///
/// # fn main() {
/// let _: AnyValue = AnyValue::i32(6);
/// # }
/// ```
///
/// ## `for<..>` entries
///
/// Parameters declared in a `for<..>` binder are the entry's own and never reach the enum. Each is
/// passed to the trait as an argument, and the parameter it lands on, carrying the bounds the
/// binder gave it, is what the enum declares:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # use closed_trait::Enumerable;
/// struct Boxed<U>(U);
///
/// #[enumerate]
/// #[sealed(for<U> Boxed<U>: Store<U>)] // here `U` is used in place of `T` declared by Store
/// trait Store<T> {}
///
/// impl<V> Store<V> for Boxed<V> {}
///
/// # fn main() {
/// let _: AnyStore<u8> = Boxed(1u8).into_enum();
/// # }
/// ```
///
/// A name never passed that way lands on no parameter, so the variant has nothing to be generic
/// over and the entry is refused: under a trait declaring none at all, `for<U> Boxed<U>: Shape`
/// leaves `U` free. `#[sealed]` accepts it, but the enum cannot.
///
/// ## Pinned entries and `match_any`
///
/// An entry *pins* its arguments when it names a concrete instantiation instead of the trait's
/// parameters. Such an entry becomes a variant like any other, and the enum does not record which
/// instantiation that variant belongs to.
///
/// `into_enum` and `From` exist only at the instantiations the entry named, so nothing ever builds
/// a variant that does not belong. Pinning therefore works with `enumerate`.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # use closed_trait::Enumerable;
/// struct Plain;
///
/// #[enumerate]
/// #[sealed(Plain: Store<i32>)]
/// pub trait Store<T> {}
///
/// impl Store<i32> for Plain {}
///
/// # fn main() {
/// let _: AnyStore = Plain.into_enum();
/// # }
/// ```
///
/// What such an entry rules out is `match_any`. Nothing stops the trait from being *named* at an
/// instantiation no permitted type implements, and that is precisely where a body may ask the macro
/// to expand. This is what it would become there, written out by hand:
///
/// ```compile_fail
/// # use closed_trait::{enumerate, sealed};
/// #[enumerate]
/// #[sealed(i32: Value<i32>)]
/// trait Value<T> {}
///
/// impl Value<i32> for i32 {}
///
/// fn describe(value: impl Value<String>) {
///     // what `match_any_value!(value.into_enum(), v => takes(v))` becomes
///     match value.into_enum() {
///         // error: the trait bound `i32: Value<String>` is not satisfied
///         AnyValue::i32(v) => takes(v),
///     }
/// }
///
/// fn takes<V: Value<String>>(_: V) {}
/// # fn main() {}
/// ```
///
/// The enum is fine, and so is the signature: `Value<String>` is a legal bound, merely one that
/// nothing satisfies. Drop the `match` and it compiles on its own:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # #[enumerate]
/// # #[sealed(i32: Value<i32>)]
/// # trait Value<T> {}
/// # impl Value<i32> for i32 {}
/// fn describe(_: impl Value<String>) {}
/// # fn main() {}
/// ```
///
/// What cannot hold is the `match_any`. `AnyValue::i32` hands back an `i32`, which is a `Value<i32>`
/// and nothing else, so a body written against `Value<String>` cannot use it. Rather than generate
/// that and let it fail inside the caller's code, `#[enumerate]` refuses it where the list is
/// written.
///
/// A permitted type that is not `Sized` cannot be held in a variant. That one is rustc's to
/// report rather than this macro's, since sizedness is not visible in the tokens.
#[proc_macro_attribute]
pub fn enumerate(args: TokenStream, item: TokenStream) -> TokenStream {
    enumerate::enumerate(args, item)
}

/// Generates a macro that instantiates the attributed function for a type, when that type satisfies
/// the bounds on the function's first parameter.
///
/// The macro is named after the function, with a `try_` prefix: `describe` gets `try_describe!`,
/// `parse` gets `try_parse!`. `name = ".."` gives it another name. It takes an expression, tests
/// its type, and returns `Some` of an [`Fn`] for that instantiation or `None`. Nothing is moved and
/// nothing runs until that `Fn` is called, which may happen more than once:
///
/// ```
/// # use closed_trait::if_implements_fn;
/// # use std::fmt::Display;
/// #[if_implements_fn]
/// fn into_i32_plus_one(v: impl Into<i32>) -> i32 {
///     v.into() + 1
/// }
///
/// # fn main() {
/// let n = 7u8;
/// match try_into_i32_plus_one!(n) {
///     Some(f) => assert_eq!(f(n), 8),
///     None => unreachable!("u8 implements Into<i32>"),
/// }
///
/// let s = "hello";
/// match try_into_i32_plus_one!(s) {
///     Some(_) => unreachable!("&str does not implement Into<i32>"),
///     None => {},
/// }
/// # }
/// ```
///
/// # The annotated function
///
/// The first parameter is the one tested, and its type is written in one of two ways:
///
/// ```
/// # use closed_trait::if_implements_fn;
/// # use std::fmt::Debug;
/// #[if_implements_fn]
/// fn named<T: Debug>(v: &T) {
///     println!("{v:?}");
/// }
///
/// #[if_implements_fn]
/// fn anonymous(v: &impl Debug) {
///     println!("{v:?}");
/// }
/// # fn main() {}
/// ```
///
/// Both test the same thing: the bounds written on that parameter (here `Debug`). Bounds on any
/// other parameter are not tested.
///
/// Everything else the signature says is the compiler's to check rather than this macro's, so
/// lifetimes, `where` clauses, further parameters and `async` all work as they do on any function.
/// What is refused is:
///
/// - a method, since the macro sits beside the function and a `macro_rules!` cannot be defined in
///   an `impl` or a `trait`;
/// - a function with no parameters, there being nothing to test;
/// - an `unsafe fn`, whose unsafety the safe call would hide;
/// - an explicit ABI, which the ordinary call would belie;
/// - anything that would not be a valid function without the attribute, which the compiler reports
///   as it always would.
///
/// A `const fn` is accepted and keeps its constness, since it is emitted as written. The macro's
/// own path is not const (what it hands back is an `Fn` called at run time), so the macro cannot be
/// used in a const context.
///
/// The macro calls the function rather than carrying a copy of its body, which is what makes its
/// paths resolve at the *call site*: the function itself, and the traits its bounds name, have to
/// be in scope there.
///
/// # Type and const arguments
///
/// The parameters the function declares besides the first can be given after a `;`, in declaration
/// order, or left to inference as they would be at any other call:
///
/// ```
/// # use closed_trait::if_implements_fn;
/// # use std::fmt::Debug;
/// #[if_implements_fn]
/// fn function<T, const N: usize>(_: impl Debug) {}
///
/// # fn main() {
/// // `T` and `N` are named nowhere in the signature, so nothing can infer them.
/// try_function!("a"; String, 7);
/// # }
/// ```
///
/// The list is handed to a turbofish as written, so `_` and const arguments work as they do there,
/// `try_function!("a"; String, { 3 + 4 })` included.
///
/// # Options
///
/// Each is a `key = "value"` pair, written in any order, and written at most once unless said
/// otherwise.
///
/// ## Visibility
///
/// `vis = ".."` gives the macro a visibility of its own, written as it would be on any item. Left
/// out, it takes the function's, capped at the crate: a `pub` function gets a `pub(crate)` macro,
/// and anything narrower keeps what it has. Written out, it may narrow the function's visibility
/// but not widen it, since the expansion is a call to the function: a macro reaching further than
/// the function it calls would fail only at the call site.
///
/// ```
/// # use closed_trait::if_implements_fn;
/// # use std::fmt::Debug;
/// #[if_implements_fn(vis = "pub(self)")]
/// pub fn print_debug<T: Debug>(v: &T) {
///     println!("{v:?}");
/// }
/// # fn main() {}
/// ```
///
/// **`vis = "pub"` is the one value refused: the macro does not leave the crate, however public the
/// function is.** A `macro_rules!` leaves its crate only through `#[macro_export]`, and that plants
/// its name in the crate *root* rather than in the module it was written in, while the expansion
/// still calls the function by the name it was written under.
///
/// ```
/// mod private {
///     pub fn not_pub() {}
/// }
/// # fn main() {}
/// ```
///
/// The `pub` on the function above is meaningless, since its enclosing module is private. Exporting
/// its macro would make that reachable from any crate while `not_pub` stays unreachable outside its
/// module. An attribute is handed the item alone and never its surroundings, so there is no way to
/// detect this, and for that reason exporting is not allowed at all.
///
/// ## Naming
///
/// `name = ".."` is the macro's whole name, `try_` included, for a function whose macro reads badly
/// under the prefix or whose name is already taken:
///
/// ```
/// # use closed_trait::if_implements_fn;
/// # use std::fmt::Debug;
/// #[if_implements_fn(name = "debug_if_possible")]
/// pub fn print_debug<T: Debug>(v: &T) {
///     println!("{v:?}");
/// }
///
/// # fn main() {
/// assert!(debug_if_possible!(&1).is_some());
/// # }
/// ```
///
/// # In a generic function
///
/// The bounds are tested against what the expression's type is *known* to be where the macro is
/// written. Inside a generic function that is whatever the function declares, and not what it is
/// later called with:
///
/// ```
/// # use closed_trait::if_implements_fn;
/// # use std::fmt::Display;
/// #[if_implements_fn]
/// fn show(v: impl Display) -> String {
///     format!("{v}")
/// }
///
/// fn unbounded<T>(v: T) -> bool {
///     try_show!(v).is_some()
/// }
///
/// fn bounded<T: Display>(v: T) -> bool {
///     try_show!(v).is_some()
/// }
///
/// # fn main() {
/// assert!(!unbounded(6u8)); // `u8` is `Display`, but `T` is not
/// assert!(bounded(6u8));
/// # }
/// ```
///
/// `unbounded` answers `None` for every `T`, `u8` included: nothing there says `T: Display`, so the
/// test cannot hold. Declaring the bound is what makes it hold, and once it is declared the call
/// could be written directly. So this is for a call site that knows the type concretely.
///
/// # Rationale
///
/// On its own the macro is rarely useful, since whether the tested type implements the given bounds
/// is decided statically: most of the time the whole mechanism folds into either the direct call,
/// when they hold, or nothing when they do not.
///
/// What it really buys is a call that can be written safely where the first argument (the one whose
/// type is tested) may not have the right type. That is especially useful inside a
/// [`match_any`][macro@enumerate] invocation, where the expression assumes several distinct types.
///
/// It is what lets that call be written once, inside the arm:
///
/// ```
/// # use closed_trait::{enumerate, if_implements_fn, sealed};
/// # use closed_trait::Enumerable;
/// # use std::fmt::Debug;
/// # #[derive(Debug)]
/// # struct ImplDebug;
/// # struct NotDebug;
/// #[enumerate(match_any)]
/// #[sealed(ImplDebug, NotDebug)]
/// trait MyTrait {}
/// # impl MyTrait for ImplDebug {}
/// # impl MyTrait for NotDebug {}
///
/// #[if_implements_fn]
/// fn print_debug(e: impl Debug) {
///     println!("{e:?}");
/// }
///
/// # fn main() {
/// let v = ImplDebug.into_enum();
/// match_any_my_trait!(v, v => {
///     if let Some(f) = try_print_debug!(v) {
///         f(v);
///     }
/// });
/// # }
/// ```
///
/// Written by hand, the same call does not compile:
///
/// ```compile_fail
/// # use closed_trait::{enumerate, if_implements_fn, sealed};
/// # use closed_trait::Enumerable;
/// # use std::fmt::Debug;
/// # #[derive(Debug)]
/// # struct ImplDebug;
/// # struct NotDebug;
/// # #[enumerate(match_any)]
/// # #[sealed(ImplDebug, NotDebug)]
/// # trait MyTrait {}
/// # impl MyTrait for ImplDebug {}
/// # impl MyTrait for NotDebug {}
/// # fn print_debug(e: impl Debug) {
/// #     println!("{e:?}");
/// # }
/// # fn main() {
/// let v = ImplDebug.into_enum();
/// match_any_my_trait!(v, v => {
///     print_debug(v); // compile error: NotDebug does not implement the Debug trait
/// });
/// # }
/// ```
///
/// That can be fixed by not calling `print_debug` on `NotDebug`, but only by writing the whole
/// match out, which grows tedious once the enum has dozens of variants.
#[proc_macro_attribute]
pub fn if_implements_fn(args: TokenStream, item: TokenStream) -> TokenStream {
    implements::if_implements_attribute(args, item)
}
