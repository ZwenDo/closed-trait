//! The `#[enumerate]` options that change names or add attributes.

use closed_trait::{enumerate, sealed};

/// Somewhere other than `::closed_trait` to find `Enumerable`.
mod reexport {
    pub use closed_trait::*;
}

// The enum derives these, so what it holds has to as well.
#[derive(Debug, Clone, PartialEq)]
pub struct Square;
#[derive(Debug, Clone, PartialEq)]
pub struct Circle;

#[enumerate(
    match_any,
    name = Shapes,
    owned(attrs = "#[derive(Debug, Clone, PartialEq)]"),
    crate = "crate::reexport"
)]
#[sealed(Square, Circle)]
pub trait Shape {
    fn area(&self) -> i32;
}

impl Shape for Square {
    fn area(&self) -> i32 {
        1
    }
}
impl Shape for Circle {
    fn area(&self) -> i32 {
        2
    }
}

#[test]
fn the_enum_takes_the_given_name() {
    let shape: Shapes = Square.into();
    assert_eq!(match_any_shape!(&shape, s => s.area()), 1);
}

#[test]
fn attrs_land_on_the_enum_verbatim() {
    let shape = Shapes::from(Circle);
    // `Debug`, `Clone` and `PartialEq` all came from `attrs`
    assert_eq!(format!("{:?}", shape.clone()), "Circle(Circle)");
    assert!(shape == Shapes::from(Circle));
}

#[test]
fn the_match_macro_uses_the_renamed_enum() {
    let shape = Shapes::from(Circle);
    assert_eq!(match_any_shape!(&shape, s => s.area()), 2);
}

/// The borrowing enum follows the enum's name rather than the trait's.
#[test]
fn the_ref_enum_follows_the_renamed_enum() {
    let square = Square;
    let borrowed: ShapesRef<'_> = ShapesRef::from(&square);
    assert_eq!(match_any_shape_ref!(borrowed, s => s.area()), 1);
}
