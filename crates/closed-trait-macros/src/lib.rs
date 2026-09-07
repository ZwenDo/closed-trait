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
/// Each entry is a type, and can further say how that type implements the trait.
///
/// ## Generic traits and types
///
/// One rule governs everything in this section:
///
/// **A bare name is a parameter only if the trait or a `for<..>` declares it**; otherwise it is
/// whatever concrete type or const is in scope. The three cases below are the three ways an entry
/// can answer that, and each takes lifetimes, types and const parameters alike.
///
/// ### Parameters the trait declares
///
/// A generic type implementing a generic trait at the same parameters names them as the trait
/// declares them:
///
/// ```
/// # use closed_trait::sealed;
/// #[sealed(Boxed<'a, T>)] // `'a` and `T` are declared by the `Store` trait
/// trait Store<'a, T> {}
///
/// struct Boxed<'t, X>(&'t X);
///
/// impl<'t, X> Store<'t, X> for Boxed<'t, X> {}
/// # fn main() {}
/// ```
///
/// Note which names the entry uses: `Boxed` declares `'t` and `X`, and the entry still writes `'a`
/// and `T`. A bare name in an entry is read against the *trait*, never against the type it belongs
/// to. A const parameter is named the same way, so `Row<N>` under `trait Width<const N: usize>`
/// means every `Row`.
///
/// ### One instantiation
///
/// An implementor may implement a generic trait at one instantiation rather than generically. The
/// `Entry: Trait<..>` syntax says which:
///
/// ```
/// # use closed_trait::sealed;
/// struct Plain;
/// struct Boxed<T>(pub T);
/// struct Keyed<T>(pub T);
///
/// #[sealed(
///     Plain: Store<i32>,        // implements the trait at one instantiation
///     Boxed<T>,                 // the identity mapping needs no annotation
///     Keyed<T>: Store<Vec<T>>,  // generic, but not the identity mapping
/// )]
/// trait Store<T> {}
///
/// impl Store<i32> for Plain {}
/// impl<T> Store<T> for Boxed<T> {}
/// impl<T> Store<Vec<T>> for Keyed<T> {}
/// # fn main() {}
/// ```
///
/// ### Parameters the trait does not declare
///
/// A type may be generic over parameters the trait knows nothing about. The entry declares them
/// itself, with `for<..>`:
///
/// ```
/// # use closed_trait::sealed;
/// struct Boxed<T>(T);
///
/// #[sealed(for<T> Boxed<T>)]
/// trait Shape {}
///
/// impl<T> Shape for Boxed<T> {}
/// # fn main() {}
/// ```
///
/// Lifetimes work the same way, except that for them the binder is not optional. Left out, the
/// same spelling would mean the trait's lifetime or every lifetime depending on what the trait
/// happened to call its parameter, so renaming that parameter would quietly change what is
/// sealed:
///
/// ```
/// # use closed_trait::sealed;
/// struct Str<'a>(&'a str);
///
/// #[sealed(for<'a> Str<'a>)]
/// trait Shape {}
///
/// impl<'a> Shape for Str<'a> {}
/// # fn main() {}
/// ```
///
/// Lifetimes, types and const parameters can be declared together, lifetimes first (as in
/// `for<'a, T: Clone, const N: usize>`), and each is written exactly as it would be on an `impl`,
/// so a const parameter carries its type.
///
/// ## `as Name`
///
/// Names the entry. The seal itself does not care: it is [`enumerate`][macro@enumerate] that reads
/// the name, giving each variant the type's last path segment unless one is written here. Two
/// entries collide over that in two ways.
///
/// **Different types whose last segment matches.** Here the name settles which is which:
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// mod a { pub struct Foo; }
/// mod b { pub struct Foo; }
///
/// #[enumerate]
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
/// #[sealed(Plain: Store<i32>, Plain as PlainF64: Store<f64>, Boxed<T>)]
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
/// some entry (`Boxed<T>` here) has to *mention* `T`. The enum is generic over the parameters
/// its variants use, not over the trait's, since an enum may not declare one no variant uses. With
/// every entry pinned there would be no `AnyStore<T>` at all, both entries would land in the same
/// `AnyStore`, and `enumerate` would refuse it.
///
/// ## All of it at once
///
/// A binder, the type, a name and the instantiation it implements, in that order:
///
/// ```
/// # use closed_trait::sealed;
/// struct Foo<'a, T>(&'a T);
///
/// #[sealed(
///     for<'a, T> Foo<'a, T> as Bar: Dummy<i32>
/// )]
/// trait Dummy<X> {}
///
/// impl<'a, T> Dummy<i32> for Foo<'a, T> {}
/// # fn main() {}
/// ```
///
/// # The list is checked in both directions
///
/// Every entry is checked, which is why the trait's type and const parameters have to be supplied
/// for it: either the type names them itself, as `Boxed<T>` does under `trait Store<T>`, or the
/// entry annotates its instantiation, as in `Plain: Store<i32>`. An entry that does neither is
/// refused, since nothing could then tell whether it implements the trait at all.
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
/// An entry naming the parameters instead, like `Boxed<T>`, permits every instantiation, which is
/// what naming them says. Lifetimes are not on the marker, for the reason above: they could never
/// tell two entries apart.
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
/// Grouped, it is a **base** that each kind extends, so `name = Shapes` gives `Shapes`, `ShapesRef`
/// and `ShapesMut`. Specific, it is the name itself: `ref(name = ShapeView)` gives exactly
/// `ShapeView`.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
/// # struct Square;
///
/// #[enumerate(name = Shapes, ref(name = ShapeView))]
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
/// The option takes an optional name, so `match_any(match_shape)` generates `match_shape!` instead.
/// Whether the macro can leave the crate depends on the trait's visibility, and so does whether
/// it can collide, see [Visibility](#visibility).
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
/// their hidden root names would be the same, and one of them needs `match_any(other_name)`.
///
/// # Generics
///
/// The enum takes the trait's parameters that at least one entry names, with their bounds. Those
/// parameters have to be nameable in the supertrait bound that pins the enum, and only the
/// trait's own are in scope there, so an entry must be generic *solely* over parameters the trait
/// declares.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
///
/// struct Boxed<T>(T);
/// struct Listed<T>(Vec<T>);
///
/// #[enumerate(match_any)]
/// #[sealed(Boxed<T>, Listed<T>)]
/// pub trait Store<T> {}
///
/// impl<T> Store<T> for Boxed<T> {}
/// impl<T> Store<T> for Listed<T> {}
///
/// # fn main() {
/// let _: AnyStore<u8> = Listed(vec![]).into();
/// # }
/// ```
///
/// The enum is generic over the parameters its *variants* use, not over the trait's. An enum
/// declaring one that no variant uses is refused, so a list whose every entry fixes its arguments
/// produces a plain enum rather than a generic one.
///
/// ```
/// # use closed_trait::{enumerate, sealed};
///
/// // every entry fixes its argument, so `AnyValue` is a plain enum
/// #[enumerate]
/// #[sealed(i32: Value<i32>, f64: Value<f64>)]
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
/// An entry that names no parameter the enum is generic over cannot produce a single enum type, and
/// is rejected with a message naming the fix: annotate it in `#[sealed(..)]` with the
/// instantiation it implements.
///
/// ## Pinned entries and `match_any`
///
/// An entry *pins* its arguments when it names a concrete instantiation instead of the trait's
/// parameters. Such an entry becomes a variant like any other, and the enum does not record which
/// instantiation that variant belongs to.
///
/// On the way *in* that costs nothing: `into_enum` and `From` exist only at the instantiations the
/// entry named, so nothing ever builds a variant that does not belong.
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
/// On the way *out* it costs the macro. Nothing stops the trait from being *named* at an
/// instantiation no permitted type implements, and that is precisely where a body may ask the
/// macro to expand. This is what `match_any` would become there, written out by hand:
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
/// What cannot hold is the `match`. `AnyValue::i32` hands back an `i32`, which is a `Value<i32>`
/// and nothing else, so a body written against `Value<String>` cannot use it. Rather than generate
/// that and let it fail inside the caller's code, `#[enumerate]` refuses it where the list is
/// written.
///
/// None of this is a trade you elect. Pinning is the only way to put such a type in the enum at
/// all: an entry that neither names the trait's parameters nor fixes them is refused outright, so
/// the macro is not something you give up in exchange, it is simply unavailable once a variant
/// exists that is not valid at every instantiation. `match_any` needs every entry to name the
/// trait's parameters rather than fix them, which is exactly the case where every variant is valid
/// everywhere.
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
/// its type, and returns `Some` of an[`Fn`] for that instantiation or `None`. Nothing is moved and
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
/// and anything narrower keeps what it has.
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
/// `name = ".."` replaces the `try_` prefix outright, for a function whose macro reads badly under it
/// or whose name is already taken:
///
/// ```
/// # use closed_trait::if_implements_fn;
/// # use std::fmt::Debug;
/// #[if_implements_fn(name = "debug_if_possible")]
/// pub fn print_debug<T: Debug>(v: &T) {
///     println!("{v:?}");
/// }
/// # fn main() {}
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
