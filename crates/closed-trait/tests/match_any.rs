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

/// One macro covers all three modes: match ergonomics decide whether the
/// binding is `&Square`, `&mut Square` or `Square`.
#[test]
fn every_mode() {
    let shape = AnyShape::from(Square { side: 3 });
    assert_eq!(match_any_shape!(&shape, s => s.area()), 9);

    let mut shape = AnyShape::from(Square { side: 3 });
    match_any_shape!(&mut shape, s => s.grow());
    assert_eq!(match_any_shape!(&shape, s => s.area()), 16);

    let shape = AnyShape::from(Circle { radius: 2 });
    assert_eq!(match_any_shape!(shape, s => s.area()), 12);
}

/// The body owns what it moves, because only one arm ever runs. This is the
/// case a visitor holding its captures behind `&mut self` could not express.
#[test]
fn the_body_may_move_what_it_owns() {
    let shape = AnyShape::from(Square { side: 3 });
    let prefix = String::from("area=");

    let described = match_any_shape!(&shape, s => prefix + &s.area().to_string());
    assert_eq!(described, "area=9");
}

/// `return` leaves the enclosing function, which a visitor method cannot do.
fn first_big(shapes: &[AnyShape]) -> Option<i32> {
    for shape in shapes {
        match_any_shape!(shape, s => {
            if s.area() > 9 {
                return Some(s.area());
            }
        });
    }
    None
}

#[test]
fn early_return_escapes_the_body() {
    let shapes = [
        AnyShape::from(Square { side: 3 }),
        AnyShape::from(Circle { radius: 2 }),
    ];
    assert_eq!(first_big(&shapes), Some(12));
}

/// Special-casing a variant is a plain match with the macro in the last arm,
/// rather than an override.
#[test]
fn fallthrough_special_cases_a_variant() {
    let free = |shape: &AnyShape| match shape {
        AnyShape::Square(_) => 0,
        other => match_any_shape!(other, s => s.area()),
    };

    assert_eq!(free(&AnyShape::from(Square { side: 3 })), 0);
    assert_eq!(free(&AnyShape::from(Circle { radius: 2 })), 12);
}

/// Accumulating across many elements is a local and a loop, not a visitor
/// value carrying state.
#[test]
fn accumulating_needs_no_value() {
    let shapes = [
        AnyShape::from(Square { side: 2 }),
        AnyShape::from(Circle { radius: 1 }),
    ];

    let mut total = 0;
    for shape in &shapes {
        total += match_any_shape!(shape, s => s.area());
    }
    assert_eq!(total, 7);
}

/// The one thing that still wants a trait — a generic body as a parameter —
/// takes three lines of ordinary Rust, dispatched by the macro.
trait ShapeVisitor {
    fn visit<S: Shape>(&mut self, shape: &S) -> i32;
}

fn sum_with<V: ShapeVisitor>(shapes: &[AnyShape], visitor: &mut V) -> i32 {
    shapes
        .iter()
        .map(|shape| match_any_shape!(shape, s => visitor.visit(s)))
        .sum()
}

#[test]
fn a_hand_written_trait_still_composes() {
    struct Doubling;
    impl ShapeVisitor for Doubling {
        fn visit<S: Shape>(&mut self, shape: &S) -> i32 {
            shape.area() * 2
        }
    }

    let shapes = [
        AnyShape::from(Square { side: 2 }),
        AnyShape::from(Circle { radius: 1 }),
    ];
    assert_eq!(sum_with(&shapes, &mut Doubling), 14);
}

/// A renamed macro, reached by path from a module defined before the one that
/// generates it.
mod caller {
    use crate::surfaces::{AnySurface, Surface};

    pub fn cells(surface: &AnySurface) -> i32 {
        crate::surfaces::match_surface!(surface, s => s.cells())
    }
}

mod surfaces {
    use closed_trait::{enumerate, sealed};

    pub struct Tile;

    #[enumerate(match_any(match_surface))]
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
fn renamed_and_reached_by_path() {
    use surfaces::{AnySurface, Tile};
    assert_eq!(caller::cells(&AnySurface::from(Tile)), 4);
}

/// A trait too narrow to leave the crate takes no crate-root name: the macro
/// stays where it was written, reachable by path all the same.
mod restricted {
    use closed_trait::{enumerate, sealed};

    pub(crate) struct Dot;

    #[enumerate(match_any)]
    #[sealed(Dot)]
    pub(crate) trait Point {
        fn x(&self) -> i32;
    }

    impl Point for Dot {
        fn x(&self) -> i32 {
            7
        }
    }
}

#[test]
fn a_restricted_trait_is_not_hoisted() {
    use restricted::{AnyPoint, Dot, Point, match_any_point};
    assert_eq!(match_any_point!(&AnyPoint::from(Dot), p => p.x()), 7);
}

/// The same for a fully private one, where the alias is redundant with the
/// macro's own textual scope and must not warn.
mod hidden {
    use closed_trait::{enumerate, sealed};

    struct Blip;

    #[enumerate(match_any)]
    #[sealed(Blip)]
    trait Signal {
        fn level(&self) -> i32;
    }

    impl Signal for Blip {
        fn level(&self) -> i32 {
            3
        }
    }

    pub(crate) fn read() -> i32 {
        match_any_signal!(&AnySignal::from(Blip), s => s.level())
    }
}

#[test]
fn a_private_trait_is_not_hoisted() {
    assert_eq!(hidden::read(), 3);
}
