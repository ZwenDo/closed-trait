//! Generic traits and lifetimes reaching the enum and the match macro, rather
//! than stopping at `#[sealed]`.

use closed_trait::{enumerate, sealed};

pub struct Boxed<T>(pub T);
pub struct Pair<T>(pub T, pub T);

#[enumerate(match_any)]
#[sealed(for<T> Boxed<T>: Store<T>, for<T> Pair<T>: Store<T>)]
pub trait Store<T> {
    fn first(&self) -> &T;
}

impl<T> Store<T> for Boxed<T> {
    fn first(&self) -> &T {
        &self.0
    }
}

impl<T> Store<T> for Pair<T> {
    fn first(&self) -> &T {
        &self.0
    }
}

#[test]
fn the_enum_carries_the_traits_parameter() {
    let store: AnyStore<i32> = Boxed(7).into();
    assert_eq!(*match_any_store!(&store, s => s.first()), 7);

    let store = AnyStore::from(Pair("a", "b"));
    assert_eq!(*match_any_store!(&store, s => s.first()), "a");
}

#[test]
fn the_macro_matches_a_generic_enum() {
    let store = AnyStore::from(Pair(3, 4));
    assert_eq!(*match_any_store!(&store, s => s.first()), 3);
}

pub struct Borrowed<'a>(pub &'a str);

#[enumerate(match_any)]
#[sealed(for<'a> Borrowed<'a>: Borrow<'a>)]
pub trait Borrow<'a> {
    fn text(&self) -> &'a str;
}

impl<'a> Borrow<'a> for Borrowed<'a> {
    fn text(&self) -> &'a str {
        self.0
    }
}

#[test]
fn the_enum_carries_a_lifetime() {
    let owned = String::from("borrowed");
    let borrow = AnyBorrow::from(Borrowed(&owned));
    assert_eq!(match_any_borrow!(&borrow, b => b.text()), "borrowed");
}

#[test]
fn a_trait_with_its_own_lifetime_still_borrows() {
    // The trait already declares `'a`, so the borrowing enums had to pick a
    // different name for the lifetime of the borrow itself.
    let owned = String::from("borrowed");
    let mut any = AnyBorrow::from(Borrowed(&owned));

    assert_eq!(
        match_any_borrow_ref!(any.as_ref(), b => b.text()),
        "borrowed"
    );
    assert_eq!(
        match_any_borrow_mut!(any.as_mut(), b => b.text()),
        "borrowed"
    );

    // and from a plain reference. On a concrete type the trait has to be
    // imported; a generic `S: Borrow<'_>` would get it from the supertrait.
    use closed_trait::EnumerableRef;
    let borrowed = Borrowed(&owned);
    assert_eq!(
        match_any_borrow_ref!(borrowed.as_enum_ref(), b => b.text()),
        "borrowed"
    );
}

#[test]
fn a_generic_trait_lends_uniquely() {
    let mut store = AnyStore::from(Pair(3, 4));
    assert_eq!(*match_any_store_mut!(store.as_mut(), s => s.first()), 3);
}

/// A trait that spends both `'a` and `'r` leaves the borrowing enums to find a
/// third name for the lifetime of the borrow itself.
mod crowded_lifetimes {
    use closed_trait::{enumerate, sealed};

    pub struct Pair<'a, 'r>(pub &'a str, pub &'r str);

    #[enumerate(match_any)]
    #[sealed(for<'a, 'r> Pair<'a, 'r>: Held<'a, 'r>)]
    pub trait Held<'a, 'r> {
        fn first(&self) -> &'a str;
        fn second(&self) -> &'r str;
    }

    impl<'a, 'r> Held<'a, 'r> for Pair<'a, 'r> {
        fn first(&self) -> &'a str {
            self.0
        }
        fn second(&self) -> &'r str {
            self.1
        }
    }
}

#[test]
fn the_borrow_finds_a_name_the_trait_has_not_spent() {
    use crowded_lifetimes::{AnyHeld, AnyHeldRef, Held, Pair, match_any_held_ref};

    let (a, b) = (String::from("x"), String::from("y"));
    let owned = AnyHeld::from(Pair(&a, &b));
    let borrowed: AnyHeldRef<'_, '_, '_> = owned.as_ref();
    assert_eq!(match_any_held_ref!(borrowed, p => p.first()), "x");
    assert_eq!(match_any_held_ref!(borrowed, p => p.second()), "y");
}

/// The assertion `#[sealed]` writes declares a parameter of its own to stand for
/// the implementor. A trait that has already spent that name must not collide
/// with it.
mod crowded_types {
    use super::*;

    pub struct One<T>(pub T);
    pub struct Two<A, B>(pub A, pub B);

    // `S` is the name the assertion reaches for first.
    #[sealed(for<S> One<S>: Held<S>)]
    pub trait Held<S> {
        fn held(&self) -> &S;
    }

    impl<S> Held<S> for One<S> {
        fn held(&self) -> &S {
            &self.0
        }
    }

    // `S` and `S2` are both spent, so it has to reach further still.
    #[sealed(for<S, S2> Two<S, S2>: Paired<S, S2>)]
    pub trait Paired<S, S2> {
        fn left(&self) -> &S;
        fn right(&self) -> &S2;
    }

    impl<S, S2> Paired<S, S2> for Two<S, S2> {
        fn left(&self) -> &S {
            &self.0
        }
        fn right(&self) -> &S2 {
            &self.1
        }
    }
}

#[test]
fn the_assertion_finds_a_name_the_trait_has_not_spent() {
    use crowded_types::{Held, One, Paired, Two};

    assert_eq!(*One(1u8).held(), 1);

    let pair = Two(2u8, 'x');
    assert_eq!(*pair.left(), 2);
    assert_eq!(*pair.right(), 'x');
}

/// A `for<..>` entry is held by the enum when the instantiation says which of
/// the trait's parameters each bound name stands for. The variant is then
/// written in the trait's own parameters, which is the only thing the supertrait
/// bound naming the enum can say.
mod bound_entries {
    use super::*;

    pub struct Held<U>(pub U);
    pub struct Wrapped<X>(pub X);
    pub struct Plain;

    #[enumerate(match_any)]
    #[sealed(
        for<U> Held<U>: Keep<U>,
        for<U> Wrapped<Vec<U>>: Keep<U>,
        for<U> Plain: Keep<U>,
    )]
    pub trait Keep<T> {
        fn count(&self) -> usize;
    }

    impl<U> Keep<U> for Held<U> {
        fn count(&self) -> usize {
            1
        }
    }

    impl<U> Keep<U> for Wrapped<Vec<U>> {
        fn count(&self) -> usize {
            self.0.len()
        }
    }

    impl<U> Keep<U> for Plain {
        fn count(&self) -> usize {
            0
        }
    }
}

#[test]
fn a_bound_entry_reaches_the_enum_in_the_traits_parameters() {
    use bound_entries::{AnyKeep, Held, Keep, Plain, Wrapped, match_any_keep};
    use closed_trait::Enumerable;

    // The instantiation is named because `Plain` implements `Keep<U>` at every
    // `U`, which leaves a bare `k.count()` with no way to pick one. That is what
    // an entry generic over the trait's parameter means, not a wrinkle of the
    // enum.

    // `for<U> Held<U>: Keep<U>` became the variant `Held<T>`.
    let held: AnyKeep<u8> = Held(1u8).into_enum();
    assert_eq!(match_any_keep!(held, k => Keep::<u8>::count(&k)), 1);

    // `Wrapped<Vec<U>>` became `Wrapped<Vec<T>>`, so the substitution reaches
    // inside nested arguments.
    let wrapped: AnyKeep<u8> = Wrapped(vec![1u8, 2, 3]).into_enum();
    assert_eq!(match_any_keep!(wrapped, k => Keep::<u8>::count(&k)), 3);

    // The type names nothing the binder bound, so it is carried as written.
    let plain: AnyKeep<u8> = Plain.into_enum();
    assert_eq!(match_any_keep!(plain, k => Keep::<u8>::count(&k)), 0);
}

/// The mapping from a binder's names to the trait's is positional, so it holds
/// for lifetimes and const parameters as readily as types, and does not depend
/// on the two lists agreeing in order.
mod bound_shapes {
    use super::*;

    pub struct Slice<'b>(pub &'b str);

    #[enumerate]
    #[sealed(for<'b> Slice<'b>: Borrowed<'b>)]
    pub trait Borrowed<'x> {
        fn read(&self) -> &str;
    }

    impl<'b> Borrowed<'b> for Slice<'b> {
        fn read(&self) -> &str {
            self.0
        }
    }

    pub struct Arr<const N: usize>(pub [u8; N]);

    #[enumerate]
    #[sealed(for<const N: usize> Arr<N>: Sized_<N>)]
    pub trait Sized_<const M: usize> {
        fn size(&self) -> usize;
    }

    impl<const N: usize> Sized_<N> for Arr<N> {
        fn size(&self) -> usize {
            N
        }
    }

    pub struct Bounded<U>(pub U);

    // The binder bounds `U`, so only a `Debug` type is permitted, and only a
    // `Debug` type may reach the enum.
    #[enumerate]
    #[sealed(for<U: core::fmt::Debug> Bounded<U>: Guarded<U>)]
    pub trait Guarded<T> {}

    impl<U: core::fmt::Debug> Guarded<U> for Bounded<U> {}

    pub struct Pair<U, V>(pub U, pub V);

    // The instantiation places `V` first and `U` second, so the variant has to
    // come out as `Pair<B, A>` rather than `Pair<A, B>`.
    #[enumerate]
    #[sealed(for<U, V> Pair<U, V>: Swapped<V, U>)]
    pub trait Swapped<A, B> {
        fn first(&self) -> &B;
    }

    impl<U, V> Swapped<V, U> for Pair<U, V> {
        fn first(&self) -> &U {
            &self.0
        }
    }
}

#[test]
fn a_lifetime_binder_reaches_the_enum() {
    use bound_shapes::{AnyBorrowed, Borrowed, Slice};
    use closed_trait::Enumerable;

    let held = String::from("x");
    let any: AnyBorrowed<'_> = Slice(&held).into_enum();
    let AnyBorrowed::Slice(slice) = any;
    assert_eq!(slice.read(), "x");
}

#[test]
fn a_const_binder_reaches_the_enum() {
    use bound_shapes::{AnySized_, Arr, Sized_};
    use closed_trait::Enumerable;

    let any: AnySized_<3> = Arr([1, 2, 3]).into_enum();
    let AnySized_::Arr(arr) = any;
    assert_eq!(arr.size(), 3);
}

#[test]
fn the_mapping_follows_the_instantiations_order_not_the_binders() {
    use bound_shapes::{AnySwapped, Pair, Swapped};
    use closed_trait::Enumerable;

    // `Pair<u8, char>` implements `Swapped<char, u8>`, so it lands in
    // `AnySwapped<char, u8>` holding `Pair<u8, char>`.
    let any: AnySwapped<char, u8> = Pair(1u8, 'x').into_enum();
    let AnySwapped::Pair(pair) = any;
    assert_eq!(*pair.first(), 1);
}

#[test]
fn a_bound_on_a_binder_reaches_the_variants_impls() {
    use bound_shapes::{AnyGuarded, Bounded};
    use closed_trait::Enumerable;

    let any: AnyGuarded<u8> = Bounded(1u8).into_enum();
    let AnyGuarded::Bounded(held) = any;
    assert_eq!(held.0, 1);
}
