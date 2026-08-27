//! `#[enumerate]` finds its type list by looking for a sibling attribute whose
//! last path segment is `sealed`, so every way of spelling that attribute has
//! to keep working.
use closed_trait::{enumerate, sealed};

pub struct Square;
pub struct Circle;

/// Bare, as re-exported from the facade.
#[enumerate(match_any)]
#[sealed(Square, Circle)]
pub trait Bare {
    fn n(&self) -> i32;
}
impl Bare for Square {
    fn n(&self) -> i32 {
        1
    }
}
impl Bare for Circle {
    fn n(&self) -> i32 {
        2
    }
}

pub struct Wedge;

/// Fully qualified through the facade.
#[closed_trait::enumerate(match_any)]
#[closed_trait::sealed(Wedge)]
pub trait Qualified {
    fn n(&self) -> i32;
}
impl Qualified for Wedge {
    fn n(&self) -> i32 {
        3
    }
}

pub mod custom {
    pub use closed_trait::{enumerate, sealed};
}

pub struct Slab;

/// Through a re-export under a different path.
#[custom::enumerate(match_any)]
#[custom::sealed(Slab)]
pub trait Reexported {
    fn n(&self) -> i32;
}
impl Reexported for Slab {
    fn n(&self) -> i32 {
        4
    }
}

/// Straight through the macros crate, which is how the facade seals itself.
pub struct Chip;

#[closed_trait_macros::enumerate(match_any)]
#[closed_trait_macros::sealed(Chip)]
pub trait ViaMacrosCrate {
    fn n(&self) -> i32;
}
impl ViaMacrosCrate for Chip {
    fn n(&self) -> i32 {
        5
    }
}

#[test]
fn every_spelling_of_the_sealed_attribute_is_found() {
    assert_eq!(match_any_bare!(AnyBare::from(Square), s => s.n()), 1);
    assert_eq!(match_any_bare!(AnyBare::from(Circle), s => s.n()), 2);
    assert_eq!(
        match_any_qualified!(AnyQualified::from(Wedge), s => s.n()),
        3
    );
    assert_eq!(
        match_any_reexported!(AnyReexported::from(Slab), s => s.n()),
        4
    );
    assert_eq!(
        match_any_via_macros_crate!(AnyViaMacrosCrate::from(Chip), s => s.n()),
        5
    );
}
