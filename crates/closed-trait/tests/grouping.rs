//! The option grammar: grouped options set the default, specific ones override.

use closed_trait::{enumerate, sealed};

pub struct Square {
    pub side: i32,
}
pub struct Circle {
    pub radius: i32,
}

/// Bare: all three enums, and the bridges between them.
#[enumerate]
#[sealed(Square, Circle)]
pub trait Shape {
    fn area(&self) -> i32;
    fn grow(&mut self);
}

impl Shape for Square {
    fn area(&self) -> i32 {
        self.side * self.side
    }
    fn grow(&mut self) {
        self.side += 1;
    }
}

impl Shape for Circle {
    fn area(&self) -> i32 {
        3 * self.radius * self.radius
    }
    fn grow(&mut self) {
        self.radius += 1;
    }
}

#[test]
fn all_three_by_default() {
    let mut owned = AnyShape::from(Square { side: 3 });

    // owned -> shared
    match owned.as_ref() {
        AnyShapeRef::Square(s) => assert_eq!(s.area(), 9),
        AnyShapeRef::Circle(_) => unreachable!(),
    }

    // owned -> unique
    match owned.as_mut() {
        AnyShapeMut::Square(s) => s.grow(),
        AnyShapeMut::Circle(_) => unreachable!(),
    }

    assert_eq!(
        match owned.as_ref() {
            AnyShapeRef::Square(s) => s.area(),
            AnyShapeRef::Circle(_) => unreachable!(),
        },
        16
    );
}

#[test]
fn a_unique_borrow_reborrows_as_shared() {
    let mut owned = AnyShape::from(Circle { radius: 2 });
    let unique = owned.as_mut();

    match unique.as_ref() {
        AnyShapeRef::Circle(c) => assert_eq!(c.area(), 12),
        AnyShapeRef::Square(_) => unreachable!(),
    }
}

/// A grouped `match_any` name is a base each kind extends; a specific one is
/// the name itself. `ref` takes a name of its own too.
mod naming {
    use closed_trait::{enumerate, sealed};

    pub struct Tile;

    #[enumerate(match_any(walk), ref(name = TileView), mut(match_any(walk_uniquely)))]
    #[sealed(Tile)]
    pub trait Surface {
        fn cells(&self) -> i32;
    }

    impl Surface for Tile {
        fn cells(&self) -> i32 {
            4
        }
    }
}

#[test]
fn grouped_names_are_bases_and_specific_ones_are_not() {
    // Each macro names its own enum, so all of them have to be in scope.
    use naming::{
        AnySurface, AnySurfaceMut, Surface, Tile, TileView, walk, walk_ref, walk_uniquely,
    };

    let mut owned = AnySurface::from(Tile);

    // the grouped base, extended per kind
    assert_eq!(walk!(&owned, s => s.cells()), 4);

    // `ref(name = ..)` renamed the enum, so the macro still expands to it
    let view: TileView<'_> = owned.as_ref();
    assert_eq!(walk_ref!(view, s => s.cells()), 4);

    // `mut(match_any(..))` is the name itself, not a base
    assert_eq!(walk_uniquely!(owned.as_mut(), s => s.cells()), 4);
}

/// `ref(skip)` drops that enum, and with it anything that produced it.
mod skipping {
    use closed_trait::{enumerate, sealed};

    pub struct Dot;

    #[enumerate(match_any, ref(skip))]
    #[sealed(Dot)]
    pub trait Point {
        fn x(&self) -> i32;
    }

    impl Point for Dot {
        fn x(&self) -> i32 {
            7
        }
    }
}

#[test]
fn a_skipped_enum_is_not_generated() {
    use skipping::{AnyPoint, AnyPointMut, Dot, Point, match_any_point, match_any_point_mut};

    let mut owned = AnyPoint::from(Dot);
    assert_eq!(match_any_point!(&owned, p => p.x()), 7);
    assert_eq!(match_any_point_mut!(owned.as_mut(), p => p.x()), 7);
}

/// `no_bridge` leaves out the conversions between the enums. That they are
/// gone is a compile error, so it is pinned by a `ui` case; this checks the
/// rest still works.
mod unbridged {
    use closed_trait::{enumerate, sealed};

    pub struct Blip;

    #[enumerate(match_any, no_bridge)]
    #[sealed(Blip)]
    pub trait Signal {
        fn level(&self) -> i32;
    }

    impl Signal for Blip {
        fn level(&self) -> i32 {
            3
        }
    }
}

#[test]
fn no_bridge_keeps_everything_else() {
    use closed_trait::EnumerableRef;
    use unbridged::{
        AnySignal, AnySignalRef, Blip, Signal, match_any_signal, match_any_signal_ref,
    };

    let owned = AnySignal::from(Blip);
    assert_eq!(match_any_signal!(&owned, s => s.level()), 3);

    // reached through the supertrait rather than through a bridge
    assert_eq!(match_any_signal_ref!(Blip.as_enum_ref(), s => s.level()), 3);
    let _: AnySignalRef<'_> = AnySignalRef::from(&Blip);
}

/// Skipping the owned enum takes `Enumerable` and `into_enum` with it, leaving
/// the borrowing pair.
mod borrow_only {
    use closed_trait::{enumerate, sealed};

    pub struct Mote;

    #[enumerate(match_any, owned(skip), mut(skip))]
    #[sealed(Mote)]
    pub trait Speck {
        fn size(&self) -> i32;
    }

    impl Speck for Mote {
        fn size(&self) -> i32 {
            5
        }
    }
}

#[test]
fn only_the_shared_enum() {
    use borrow_only::{AnySpeckRef, Mote, Speck, match_any_speck_ref};
    use closed_trait::EnumerableRef;

    assert_eq!(match_any_speck_ref!(Mote.as_enum_ref(), s => s.size()), 5);
    let _: AnySpeckRef<'_> = AnySpeckRef::from(&Mote);
}

/// `attrs` reaches the borrowing enums too, one group at a time.
mod decorated {
    use closed_trait::{enumerate, sealed};

    #[derive(Debug)]
    pub struct Grain;

    #[enumerate(owned(attrs = "#[derive(Debug)]"), ref(attrs = "#[derive(Debug)]"))]
    #[sealed(Grain)]
    pub trait Speck {
        fn size(&self) -> i32;
    }

    impl Speck for Grain {
        fn size(&self) -> i32 {
            1
        }
    }
}

#[test]
fn attrs_reach_each_enum_separately() {
    use decorated::{AnySpeck, AnySpeckRef, Grain, Speck};

    assert_eq!(Grain.size(), 1);
    let owned = AnySpeck::from(Grain);
    assert_eq!(format!("{owned:?}"), "Grain(Grain)");
    assert_eq!(format!("{:?}", AnySpeckRef::from(&Grain)), "Grain(Grain)");
}
