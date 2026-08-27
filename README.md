# closed-trait

[![Crate version](https://img.shields.io/crates/v/closed-trait.svg "Crate version")](https://crates.io/crates/closed-trait)
[![Rust 1.85+](https://img.shields.io/badge/rustc-1.85+-blue.svg "Rust 1.85+")](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/)
[![Rust docs](https://docs.rs/closed-trait/badge.svg "Rust docs")](https://docs.rs/closed-trait)

Seal a trait to a fixed set of types, and generate an enum and match macro over them.

It provides sealing, so that only the listed types may implement the trait:

```rust,compile_fail
struct Square;
struct Circle;

#[closed_trait::sealed(Square)] // Only `Square` is permitted
trait Shape {}

impl Shape for Square {}
impl Shape for Circle {} // compile error: `Circle` is not permitted

fn main() {}
```

Adding one attribute above the trait also generates three enums, one owning and two borrowing:

```rust
struct Square;
struct Circle;

#[closed_trait::enumerate]
#[closed_trait::sealed(Square, Circle)]
trait Shape { /* .. */ }

impl Shape for Square { /* .. */ }
impl Shape for Circle { /* .. */ }

fn main() {
    let shape: AnyShape = Square.into();
    match shape {
        AnyShape::Square(s) => { /* use Square functions */ }
        AnyShape::Circle(c) => { /* use Circle functions */ }
    }
}
```

The `match_any` option adds a macro that matches every variant:

```rust
struct Square;
struct Circle;

#[closed_trait::enumerate(match_any)]
#[closed_trait::sealed(Square, Circle)]
trait Shape { fn corners(&self) -> u32; }

impl Shape for Square { fn corners(&self) -> u32 { 4 } }
impl Shape for Circle { fn corners(&self) -> u32 { 0 } }

fn main() {
    let shapes: Vec<AnyShape> = vec![Square.into(), Circle.into()];
    let mut count = 0;
    for shape in shapes {
        count += match_any_shape!(shape, s => s.corners());
    }
    assert_eq!(count, 4);
}
```

Note that `#[enumerate]` reads its type list from the `#[sealed(..)]` below it, so it goes **above**.

## Why seal a trait?

Sealing earns its place before any enum is generated. Part of a public trait is frozen the moment it ships, whatever
you do: its method signatures belong to every caller, and no seal changes that. What sealing frees is the other side: a
trait anyone can implement cannot gain a required method or a new supertrait without breaking every implementation
downstream, so its API can only ever grow by defaults. Seal it and no outside implementation exists, so both become
ordinary changes: add them and just fix your own types.

It also means you know the complete set of implementations. When a trait drives something sensitive (`unsafe` traits
being the obvious case), that is the difference between a guarantee you can review and one you can only document.

You can seal by hand, or reach for one of the crates that do it, and either way the seal is a private marker trait. The
set it defines is never written down anywhere: it exists only as `impl Sealed for ..` lines sitting beside whichever
type each one belongs to, so the only way to learn what is in the set is to go looking for them. Here the set *is* a
list, in one place, directly above the trait it seals, which a reader takes in at a glance and a macro like
`#[enumerate]` can read.

## Why not `dyn Trait`, or an enum?

Rust already has ways to say "one of a few types". Each gives something up.

**`dyn Trait`** requires the trait to be dyn compatible: no generic methods, no associated constants, no method
returning `Self`. This crate has no such requirement, and works on traits `dyn` cannot express at all. You also pay for
what a trait object is: a fat pointer, a vtable hop that blocks inlining, and a heap allocation to own one, which rules
it out where there is no `alloc`. And the concrete type is erased, so the inherent methods and fields of the type you
actually have are out of reach.

**A bare enum** has no trait at all. Its variants are not types, so there is nothing to bound on, no
`fn draw<S: Shape>(shape: S)` to write, and every variant shares a single method set, so behaviour that makes sense for
only one of them still exists on all the others.

**An enum of newtypes** can have a trait alongside it, but nothing ties the two together. A type can implement the trait
without appearing as a variant, or a variant's inner type can drift out of the trait, and the compiler will not say a
word. And to run trait-generic code over the enum you end up adding `&dyn Trait` converters, which brings back
everything above.

The trait stays a trait, the set is closed *and* known, and the list is checked in both directions. A listed type that
does not implement the trait is a compile error, so the two cannot drift apart.

## What you get

`#[sealed(..)]` alone generates a two-level private marker that only the listed types can satisfy, and a targeted error
for anything else.

`#[enumerate]` generates the rest, shown here for a `Shape` sealed to `Square` and `Circle`:

| generated                                                     | what it is                                                        |
|---------------------------------------------------------------|-------------------------------------------------------------------|
| `AnyShape`, `AnyShapeRef<'a>`, `AnyShapeMut<'a>`              | one variant per permitted type: owning, shared, unique           |
| `From<Square>`, `From<&Square>`, `From<&mut Square>`          | one per permitted type, into the matching enum                    |
| `Enumerable`, `EnumerableRef`, `EnumerableMut`                | supertraits carrying `into_enum`, `as_enum_ref` and `as_enum_mut` |
| `AnyShape::as_ref`, `AnyShape::as_mut`, `AnyShapeMut::as_ref` | between the enums, without going back through the concrete type   |
| `match_any_shape!` and its two borrowing twins                | with the `match_any` option, a match over every variant          |

The match macro is the piece a plain `match` cannot replace: it hands the body the **concrete** type. Rust has no
generic closures, so copying the body into every arm is the only way to have one body that still knows what it has.
Being a `match` rather than a call, `return` and `?` in the body leave the enclosing function, the body may move
anything it owns, and it can be `async`.

The one thing it does not generate is an `impl Shape for AnyShape`, and that is deliberate, because forwarding cannot always be
written. A trait with an associated type, `type Bar; fn make(&self) -> Self::Bar`, has no single return type to give the
enum, since every implementor picks its own. Per-arm bodies never face that, because nothing has to unify. And where
forwarding does make sense, it is one line: `match_any_shape!(self, s => s.area())` in an inherent impl.

## Constraints

Each piece asks something in return, and each rule follows from how that piece works.

**`#[sealed]` has to name your types**, so they must live at module level: the generated module refers to them by path,
and a type declared inside a function body cannot be reached from there.

**The enums hold their types by value**, so every permitted type must be `Sized`. `#[sealed]` on its own is content
with `str` or `[u8]`; it is only `#[enumerate]` that needs the bound.

**`match_any` copies the body into every arm.** That copying is exactly what gives the body a concrete type, and the
rest follows from it. The body is type-checked once per variant, a mistake in it is reported once per variant, and
nesting one macro inside another squares both. So a long body belongs in a generic function that the arm merely calls.
Code size is unaffected: a generic function is monomorphised per type either way.

**A type fixed to one instantiation cannot use `match_any`.** `#[sealed(Plain: Store<i32>)]` still gives `Plain` its
variant, but the macro expands one body across all of them, and that body has to hold at every instantiation, which a
variant fixed to `i32` does not. See [`#[enumerate]`](https://docs.rs/closed-trait/latest/closed_trait/attr.enumerate.html).

## Documentation

Both attributes are documented in full, with the entry grammar, every option, and the borrowing enums:

- [`#[sealed]`](https://docs.rs/closed-trait/latest/closed_trait/attr.sealed.html)
- [`#[enumerate]`](https://docs.rs/closed-trait/latest/closed_trait/attr.enumerate.html)

## Notes

**`no_std`**, and no `alloc` either. The proc-macro crate runs on the host and never reaches your binary.

**The seal is not merely conventional.** The marker trait is private to the module the attribute is written in, and
carries a supertrait private one level deeper, so naming the marker is not enough to satisfy it. Even code sitting
directly beside the sealed trait cannot opt a type in, which a single level of privacy would have allowed.

## License

[MIT](https://github.com/ZwenDo/closed-trait/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/ZwenDo/closed-trait/blob/main/LICENSE-APACHE), at your option.
