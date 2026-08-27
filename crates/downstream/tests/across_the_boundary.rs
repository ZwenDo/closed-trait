//! A separate crate from the one the macro was generated in.

use downstream::shapes::{AnyShape, Shape, match_any_shape};
use downstream::{Circle, Square};

#[test]
fn the_macro_crosses_a_crate_boundary() {
    let shape = AnyShape::from(Square { side: 3 });
    assert_eq!(match_any_shape!(&shape, s => s.area()), 9);

    let shape = AnyShape::from(Circle { radius: 2 });
    assert_eq!(match_any_shape!(shape, s => s.area()), 12);
}

#[test]
fn it_reaches_the_module_path_not_just_the_root() {
    let shape = AnyShape::from(Square { side: 4 });
    assert_eq!(
        downstream::shapes::match_any_shape!(&shape, s => s.area()),
        16
    );
}

/// The borrowing supertraits refer to `closed-trait` by path from the crate the
/// macro expanded in, so they are worth exercising from a different one.
#[test]
fn the_borrowing_enums_cross_the_boundary_too() {
    use closed_trait::EnumerableRef;
    use downstream::shapes::{AnyShapeRef, match_any_shape_ref};

    let square = Square { side: 3 };
    assert_eq!(match_any_shape_ref!(square.as_enum_ref(), s => s.area()), 9);

    // and through the bridge from the owned enum
    let owned = AnyShape::from(Circle { radius: 2 });
    let borrowed: AnyShapeRef<'_> = owned.as_ref();
    assert_eq!(match_any_shape_ref!(borrowed, s => s.area()), 12);
}

/// Reaching the enum from a generic borrow needs no import at all: the
/// supertrait carries the method.
fn area_of<S: downstream::shapes::Shape>(shape: &S) -> i32 {
    // The enum is named by the macro's expansion, so it has to be in scope
    // even though nothing here mentions it.
    use downstream::shapes::{AnyShapeRef, match_any_shape_ref};
    match_any_shape_ref!(shape.as_enum_ref(), s => s.area())
}

#[test]
fn generic_over_the_trait_across_the_boundary() {
    assert_eq!(area_of(&Square { side: 4 }), 16);
}
