//! Generic traits and lifetimes reaching the enum and the match macro, rather
//! than stopping at `#[sealed]`.

use closed_trait::{enumerate, sealed};

pub struct Boxed<T>(pub T);
pub struct Pair<T>(pub T, pub T);

#[enumerate(match_any)]
#[sealed(Boxed<T>, Pair<T>)]
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
#[sealed(Borrowed<'a>)]
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
    #[sealed(Pair<'a, 'r>)]
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
