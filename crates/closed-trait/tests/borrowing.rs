//! The borrowing enum: what it can reach that the owned one cannot.

use closed_trait::{enumerate, sealed};

pub struct Square {
    pub side: i32,
}
pub struct Circle {
    pub radius: i32,
}

#[enumerate(match_any)]
#[sealed(Square, Circle)]
pub trait Shape {
    fn area(&self) -> i32;
}

impl Shape for Square {
    fn area(&self) -> i32 {
        self.side * self.side
    }
}

impl Shape for Circle {
    fn area(&self) -> i32 {
        3 * self.radius * self.radius
    }
}

/// `into_enum` takes `self`, so this is impossible with the owned enum: a
/// borrow has no route to it at all.
fn corners<S: Shape>(shape: &S) -> u32 {
    match shape.as_enum_ref() {
        AnyShapeRef::Square(_) => 4,
        AnyShapeRef::Circle(_) => 0,
    }
}

fn area_of<S: Shape>(shape: &S) -> i32 {
    match_any_shape_ref!(shape.as_enum_ref(), s => s.area())
}

#[test]
fn a_reference_reaches_the_enum() {
    assert_eq!(corners(&Square { side: 1 }), 4);
    assert_eq!(corners(&Circle { radius: 1 }), 0);
    assert_eq!(area_of(&Square { side: 3 }), 9);
}

#[test]
fn built_from_a_reference_to_a_permitted_type() {
    let square = Square { side: 4 };
    let borrowed = AnyShapeRef::from(&square);
    assert_eq!(match_any_shape_ref!(borrowed, s => s.area()), 16);
}

/// The bridge between the generated enums, rather than a `From` impl.
#[test]
fn borrowed_from_the_owned_enum() {
    let owned = AnyShape::from(Circle { radius: 2 });
    assert_eq!(match_any_shape_ref!(owned.as_ref(), s => s.area()), 12);
}

/// The point of it: a view over values living in separate places, none of
/// which could be moved into an owned enum.
#[test]
fn a_heterogeneous_view_without_owning_anything() {
    let squares = [Square { side: 2 }, Square { side: 3 }];
    let circles = [Circle { radius: 1 }];

    let view: Vec<AnyShapeRef<'_>> = squares
        .iter()
        .map(AnyShapeRef::from)
        .chain(circles.iter().map(AnyShapeRef::from))
        .collect();

    let total: i32 = view
        .iter()
        .map(|shape| match_any_shape_ref!(*shape, s => s.area()))
        .sum();
    assert_eq!(total, 4 + 9 + 3);
}

/// Every field is a shared reference, so the enum is `Copy` and using it does
/// not consume it.
#[test]
fn it_is_copy() {
    let square = Square { side: 5 };
    let borrowed = AnyShapeRef::from(&square);
    assert_eq!(match_any_shape_ref!(borrowed, s => s.area()), 25);
    assert_eq!(match_any_shape_ref!(borrowed, s => s.area()), 25);
}

/// The higher-ranked supertrait does not cost dyn compatibility.
#[test]
fn dyn_still_works() {
    let shapes: [&dyn Shape; 2] = [&Square { side: 2 }, &Circle { radius: 1 }];
    let total: i32 = shapes.iter().map(|shape| shape.area()).sum();
    assert_eq!(total, 7);
}

/// A generic trait carries its parameters into the borrowing enum too.
mod generic {
    use closed_trait::{enumerate, sealed};

    pub struct Boxed<T>(pub T);

    #[enumerate(match_any)]
    #[sealed(Boxed<T>)]
    pub trait Store<T> {
        fn get(&self) -> &T;
    }

    impl<T> Store<T> for Boxed<T> {
        fn get(&self) -> &T {
            &self.0
        }
    }
}

#[test]
fn a_generic_trait_borrows_too() {
    use generic::{AnyStoreRef, Boxed, Store, match_any_store_ref};

    let boxed = Boxed(7);
    let borrowed: AnyStoreRef<'_, i32> = AnyStoreRef::from(&boxed);
    assert_eq!(*match_any_store_ref!(borrowed, s => s.get()), 7);
}

/// The mutable twin. It is deliberately *not* `Copy`, since a unique reference
/// is neither `Copy` nor `Clone`, and reaches the same places from a `&mut S`.
///
/// A second trait rather than more options on the first: two `pub` traits of
/// the same name asking for `match_any` would collide at the crate root, which
/// is what the rename slot is for.
mod solids {
    use closed_trait::{enumerate, sealed};

    pub struct Cube {
        pub side: i32,
    }
    pub struct Ball {
        pub radius: i32,
    }

    #[enumerate(match_any)]
    #[sealed(Cube, Ball)]
    pub trait Solid {
        fn volume(&self) -> i32;
        fn grow(&mut self);
    }

    impl Solid for Cube {
        fn volume(&self) -> i32 {
            self.side * self.side * self.side
        }
        fn grow(&mut self) {
            self.side += 1;
        }
    }

    impl Solid for Ball {
        fn volume(&self) -> i32 {
            4 * self.radius * self.radius * self.radius
        }
        fn grow(&mut self) {
            self.radius += 1;
        }
    }

    /// Mutating through a generic unique borrow.
    pub fn grow_twice<S: Solid>(solid: &mut S) {
        match_any_solid_mut!(solid.as_enum_mut(), s => {
            s.grow();
            s.grow();
        });
    }

    /// Both borrowing enums reachable from the one bound.
    pub fn volume_then_grow<S: Solid>(solid: &mut S) -> i32 {
        let before = match_any_solid_ref!(solid.as_enum_ref(), s => s.volume());
        grow_twice(solid);
        before
    }
}

#[test]
fn the_mut_enum_mutates_through_a_generic_borrow() {
    use solids::{Cube, volume_then_grow};

    let mut cube = Cube { side: 3 };
    assert_eq!(volume_then_grow(&mut cube), 27);
    assert_eq!(cube.side, 5);
}

#[test]
fn the_mut_enum_borrows_from_the_owned_one() {
    // `AnySolidMut` is named by the macro's expansion, so it has to be in scope.
    use solids::{AnySolid, AnySolidMut, Ball, Solid, match_any_solid, match_any_solid_mut};

    let mut owned = AnySolid::from(Ball { radius: 1 });
    match_any_solid_mut!(owned.as_mut(), s => s.grow());
    assert_eq!(match_any_solid!(&owned, s => s.volume()), 32);
}

#[test]
fn both_supertraits_leave_dyn_alone() {
    use solids::{Ball, Cube, Solid};

    let solids: [&dyn Solid; 2] = [&Cube { side: 2 }, &Ball { radius: 1 }];
    assert_eq!(solids.iter().map(|s| s.volume()).sum::<i32>(), 12);
}
