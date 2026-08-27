//! Attribute macros for the [`closed-trait`] crate. Use them through that crate,
//! which re-exports both and provides the items the generated code refers to.
//!
//! [`closed-trait`]: https://docs.rs/closed-trait
mod enumerate;
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
/// **A bare name is a parameter only if the trait or a `for<..>` declares it** — otherwise it is
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
///   Plain: Store<i32>,        // implements the trait at one instantiation
///   Boxed<T>,                 // the identity mapping needs no annotation
///   Keyed<T>: Store<Vec<T>>,  // generic, but not the identity mapping
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
/// happened to call its parameter — so renaming that parameter would quietly change what is
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
/// Lifetimes, types and const parameters can be declared together, lifetimes first — as in
/// `for<'a, T: Clone, const N: usize>` — and each is written exactly as it would be on an `impl`,
/// so a const parameter carries its type.
///
/// ## `as Name`
///
/// Names the entry. The seal itself does not care — it is [`enumerate`][macro@enumerate] that reads
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
/// fn main() {
///   let _ = AnyShape::Left(a::Foo); // see enumerate
/// }
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
/// fn main() {
///     // the one type reaching two different enum instantiations
///     assert!(matches!(Plain.into_enum(), AnyStore::<i32>::Plain(_)));
///     assert!(matches!(Plain.into_enum(), AnyStore::<f64>::PlainF64(_)));
/// }
/// ```
///
/// The name settles the *variant* only. The two entries must also pin different arguments, and
/// some entry — `Boxed<T>` here — has to *mention* `T`. The enum is generic over the parameters
/// its variants use, not over the trait's: an enum declaring one no variant uses is `E0392`. With
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
///   for<'a, T> Foo<'a, T> as Bar: Dummy<i32>
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
/// A lifetime is never asked for, and not merely because inference usually copes. A type cannot implement the same trait at two different lifetimes — two such
/// impls overlap, and coherence rejects them — so there is never more than one candidate to
/// disambiguate. `#[sealed(Plain)]` under `trait Foo<'a>` is therefore accepted *and* checked: an
/// entry implementing no `Foo` at all is still caught.
///
/// # The seal is as precise as the list
///
/// The marker carries the same type and const parameters the trait does, so `Plain: Store<i32>`
/// permits `Plain` to implement `Store<i32>` and nothing else — an unlisted `impl Store<f64> for
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
/// An entry naming the parameters instead, like `Boxed<T>`, permits every instantiation — which is
/// what naming them says. Lifetimes are not on the marker, for the reason above: they could never
/// tell two entries apart.
///
/// # What the seal is worth
///
/// The marker trait is private to the module the attribute is written in, and carries a supertrait
/// private one level deeper. Naming the marker is therefore not enough to satisfy it — the only
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
/// Reads its type list from the `#[sealed(..)]` attribute below it, so it must be written **above**
/// — attribute macros run top down, and `#[sealed]` consumes itself when it expands.
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
/// fn  main() {
///  let shape: AnyShape = Square.into_enum();
///  match shape {
///    AnyShape::Square(_) => {},
///    AnyShape::Circle(_) => {},
///  }
/// }
/// ```
///
/// # The three enums
///
/// All three are generated by default, each with one variant per entry named after the type's last
/// path segment, and each taking the trait's visibility. A type not in upper camel case therefore
/// gives a variant that is not either — `i32` yields an `i32` variant — so the enums carry
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
/// Each brings a supertrait with it — `Enumerable<AnyShape>` and the higher-ranked `for<'a>
/// EnumerableRef<'a, AnyShapeRef<'a>>` and its `Mut` counterpart — which is what makes the enums
/// reachable from a generic `S: Shape` without naming them.
///
/// The borrowing pair is what `into_enum` cannot give you: taking `self`, it needs the value moved
/// in, so a `&S` has no route to the owned enum at all. They are also cheaper to pass, being a
/// pointer and a discriminant rather than as wide as the largest permitted type. The shared one
/// derives `Clone` and `Copy`.
///
/// Conversions *between* the three — `as_ref` and `as_mut` on the owned enum, and `as_ref` on the
/// unique one, which reborrows — come as inherent methods. `no_bridge` leaves them out.
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
/// fn main() {
///   let mut owned: Shapes        = Shapes::Square(Square);
///   let mutable:   ShapesMut<'_> = owned.as_mut();
///   let reference: ShapeView<'_> = mutable.as_ref();
/// }
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
/// Attributes to put on a generated enum, verbatim — derives, `#[non_exhaustive]`, `#[repr(..)]`,
/// anything. It is the one option that **must** be specific. What is valid differs between
/// the three — the shared enum already derives `Copy`, the unique one cannot derive `Clone` at all
/// — so one spelling spread across them would be a trap. A bare `attrs` is an error saying so.
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
/// fn main() {
///  let shape = AnyShape::from(Square { side: 3 });
///  let area = match_any_shape!(shape, s => s.area());
///  assert_eq!(9, area);
/// }
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
///   for shape in shapes {
///     // `return` leaves `first_big`, which a method taking the body could
///     // never do
///     match_any_shape!(shape, s => if s.area() > value { return Some(s.area()) });
///   }
///   None
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
/// it can collide — see [Visibility](#visibility).
///
/// # Visibility
///
/// Everything generated takes the trait's own visibility: the three enums, the conversions between
/// them, and the names the match macros are reached through. There is no option to change it — the
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
/// trait's own are in scope there — so an entry must be generic *solely* over parameters the trait
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
/// fn main() {
///     let _: AnyStore<u8> = Listed(vec![]).into();
/// }
/// ```
///
/// The enum is generic over the parameters its *variants* use, not over the trait's. An enum
/// declaring one that no variant uses is `E0392`, so a list whose every entry fixes its arguments
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
/// fn main() {
///     let _: AnyValue = AnyValue::i32(6);
/// }
/// ```
///
/// An entry that names no parameter the enum is generic over cannot produce a single enum type, and
/// is rejected with a message naming the fix — annotate it in `#[sealed(..)]` with the
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
/// fn main() {
///     let _: AnyStore = Plain.into_enum();
/// }
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
/// all — an entry that neither names the trait's parameters nor fixes them is refused outright — so
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
