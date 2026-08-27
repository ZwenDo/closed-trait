use closed_trait::sealed;

pub struct Square;

pub mod shapes {
    pub struct Circle;
}

#[sealed(Square, crate::shapes::Circle)]
pub trait Shape {
    fn area(&self) -> f64;
}

impl Shape for Square {
    fn area(&self) -> f64 {
        1.0
    }
}

impl Shape for shapes::Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI
    }
}

#[test]
fn listed_types_implement_the_sealed_trait() {
    assert_eq!(Square.area(), 1.0);
    assert_eq!(shapes::Circle.area(), std::f64::consts::PI);
}

#[test]
fn the_trait_is_usable_without_naming_the_seal() {
    fn total(shapes: &[&dyn Shape]) -> f64 {
        shapes.iter().map(|shape| shape.area()).sum()
    }

    assert_eq!(total(&[&Square, &Square]), 2.0);
}

/// Existing supertraits survive, and the seal is appended to them.
#[sealed(Named)]
trait Describe: std::fmt::Debug {
    fn describe(&self) -> String {
        format!("{self:?}")
    }
}

#[derive(Debug)]
struct Named;

impl Describe for Named {}

#[test]
fn existing_supertraits_are_kept() {
    assert_eq!(Named.describe(), "Named");
}

/// Lifetimes on the trait are threaded into the generated assertion.
#[sealed(Borrowed<'_>)]
trait Borrow<'a> {
    fn text(&self) -> &'a str;
}

struct Borrowed<'a>(&'a str);

impl<'a> Borrow<'a> for Borrowed<'a> {
    fn text(&self) -> &'a str {
        self.0
    }
}

#[test]
fn traits_with_lifetimes_seal_too() {
    let source = String::from("held");
    assert_eq!(Borrowed(&source).text(), "held");
}

/// A lifetime an entry names must come from somewhere: either the trait
/// declares it, or a `for<..>` binds it. Letting the macro declare it silently
/// would make the entry mean *every* lifetime, and would make renaming the
/// trait's own parameter change what is sealed.
mod binders {
    use closed_trait::sealed;

    pub struct Slice<'a>(pub &'a str);
    pub struct Held(pub String);

    // The trait declares no lifetime, so the quantified form says so out loud.
    #[sealed(for<'a> Slice<'a>, Held)]
    pub trait Text {
        fn text(&self) -> &str;
    }

    impl<'a> Text for Slice<'a> {
        fn text(&self) -> &str {
            self.0
        }
    }

    impl Text for Held {
        fn text(&self) -> &str {
            &self.0
        }
    }

    // Here `'a` is the trait's own, so it needs no binder, and means
    // something different: this seal is tied to the trait's lifetime rather
    // than quantified over every one.
    #[sealed(Slice<'a>)]
    pub trait Quoted<'a> {
        fn quoted(&self) -> String;
    }

    impl<'a> Quoted<'a> for Slice<'a> {
        fn quoted(&self) -> String {
            format!("{:?}", self.0)
        }
    }
}

#[test]
fn a_lifetime_comes_from_the_trait_or_from_a_binder() {
    use binders::{Held, Quoted, Slice, Text};

    let owned = String::from("hi");
    assert_eq!(Slice(&owned).text(), "hi");
    assert_eq!(Held(owned.clone()).text(), "hi");
    assert_eq!(Slice(&owned).quoted(), "\"hi\"");
}

/// A trait parameterised only by lifetimes needs no instantiation annotation,
/// even when the implementor pins the lifetime rather than being generic over
/// it. Two impls differing only in a lifetime would overlap, so there is never
/// more than one candidate for inference to weigh.
mod lifetime_only {
    use closed_trait::sealed;

    #[sealed(Pinned, Any)]
    pub trait Held<'a> {
        fn held(&self) -> i32;
    }

    pub struct Pinned;
    pub struct Any;

    // one pins the lifetime, the other is generic over it
    impl Held<'static> for Pinned {
        fn held(&self) -> i32 {
            1
        }
    }

    impl<'a> Held<'a> for Any {
        fn held(&self) -> i32 {
            2
        }
    }
}

#[test]
fn a_lifetime_only_trait_needs_no_annotation() {
    use lifetime_only::{Any, Held, Pinned};

    assert_eq!(Pinned.held(), 1);
    assert_eq!(Any.held(), 2);
}

/// A lifetime named in an instantiation is dropped from the check rather than
/// passed along: `assert` takes the trait's lifetimes late bound, so naming one
/// explicitly would be an error in waiting.
mod annotated_lifetime {
    use closed_trait::sealed;

    #[sealed(Pinned: Kept<'static>)]
    pub trait Kept<'a> {
        fn kept(&self) -> i32;
    }

    pub struct Pinned;

    impl Kept<'static> for Pinned {
        fn kept(&self) -> i32 {
            3
        }
    }
}

#[test]
fn an_annotated_lifetime_is_harmless() {
    use annotated_lifetime::{Kept, Pinned};
    assert_eq!(Pinned.kept(), 3);
}

/// A type may implement a generic trait at several instantiations, and each
/// gets its own entry so each is checked. The marker takes no parameters, so
/// they share a single `Sealed` impl rather than colliding over it.
mod several_instantiations {
    use closed_trait::sealed;

    #[sealed(Plain: Store<i32>, Plain: Store<f64>, Fixed: Store<u8>)]
    pub trait Store<T> {
        fn get(&self) -> T;
    }

    pub struct Plain;
    pub struct Fixed;

    impl Store<i32> for Plain {
        fn get(&self) -> i32 {
            1
        }
    }

    impl Store<f64> for Plain {
        fn get(&self) -> f64 {
            2.0
        }
    }

    impl Store<u8> for Fixed {
        fn get(&self) -> u8 {
            3
        }
    }
}

#[test]
fn one_type_may_be_listed_at_several_instantiations() {
    use several_instantiations::{Fixed, Plain, Store};

    assert_eq!(Store::<i32>::get(&Plain), 1);
    assert_eq!(Store::<f64>::get(&Plain), 2.0);
    assert_eq!(Fixed.get(), 3u8);
}

/// Two entries may share a type when they pin different arguments, provided the
/// enum stays generic: some entry has to name the parameter rather than fixing
/// it, or both would map into the same enum and `into_enum` would be ambiguous.
mod shared_type_distinct_enums {
    use closed_trait::{enumerate, sealed};

    #[enumerate]
    #[sealed(Plain: Keep<i32>, Plain as PlainF64: Keep<f64>, Boxed<T>)]
    pub trait Keep<T> {}

    pub struct Plain;
    pub struct Boxed<T>(pub T);

    impl Keep<i32> for Plain {}
    impl Keep<f64> for Plain {}
    impl<T> Keep<T> for Boxed<T> {}
}

#[test]
fn a_shared_type_may_pin_different_arguments() {
    use closed_trait::Enumerable;
    use shared_type_distinct_enums::{AnyKeep, Boxed, Plain};

    // the same type reaching two different enum instantiations
    let as_i32: AnyKeep<i32> = Plain.into_enum();
    let as_f64: AnyKeep<f64> = Plain.into_enum();
    assert!(matches!(as_i32, AnyKeep::Plain(_)));
    assert!(matches!(as_f64, AnyKeep::PlainF64(_)));

    let boxed: AnyKeep<u8> = Boxed(1u8).into_enum();
    assert!(matches!(boxed, AnyKeep::Boxed(_)));
}

/// Every part of the entry grammar at once: a binder, the type, an alias and
/// the instantiation it implements.
mod every_part {
    use closed_trait::sealed;

    pub struct Shown<'x, U>(pub &'x U);

    #[sealed(for<'x, U: Clone> Shown<'x, U> as Displayed: Store<i32>)]
    pub trait Store<T> {
        fn count(&self) -> usize;
    }

    impl<'x, U: Clone> Store<i32> for Shown<'x, U> {
        fn count(&self) -> usize {
            core::mem::size_of_val(self.0)
        }
    }
}

#[test]
fn an_entry_may_use_every_part_at_once() {
    use every_part::{Shown, Store};

    let value = 7i32;
    assert_eq!(Shown(&value).count(), 4);
}

/// The seal is as precise as the list: the marker carries the trait's type and
/// const parameters, so an entry pinned to one instantiation permits that one
/// and no other. `ui/seal_rejects_unlisted_instantiation.rs` pins the refusal.
mod precise {
    use closed_trait::sealed;

    #[sealed(Plain: Store<i32>, Boxed<T>)]
    pub trait Store<T> {
        fn size(&self) -> usize;
    }

    pub struct Plain;
    pub struct Boxed<T>(pub T);

    impl Store<i32> for Plain {
        fn size(&self) -> usize {
            1
        }
    }

    // `Boxed<T>` names the parameter, so it must be permitted at every `T`.
    impl<T> Store<T> for Boxed<T> {
        fn size(&self) -> usize {
            core::mem::size_of_val(&self.0)
        }
    }
}

#[test]
fn an_entry_permits_only_the_instantiation_it_names() {
    use precise::{Boxed, Plain, Store};

    assert_eq!(Store::<i32>::size(&Plain), 1);
    assert_eq!(Boxed(2u8).size(), 1);
    assert_eq!(Boxed(3.5f64).size(), 8);
}
