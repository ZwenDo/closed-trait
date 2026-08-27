//! Const generics, in a `for<..>` binder and as a trait parameter.

use closed_trait::sealed;

pub struct Array<const N: usize>(pub [u8; N]);

#[sealed(for<const N: usize> Array<N>)]
pub trait Shape {
    fn count(&self) -> usize;
}

impl<const N: usize> Shape for Array<N> {
    fn count(&self) -> usize {
        N
    }
}

/// A const parameter the trait declares needs no binder, exactly as a type
/// parameter would not.
mod declared {
    use closed_trait::sealed;

    pub struct Row<const N: usize>(pub [u8; N]);

    #[sealed(Row<N>)]
    pub trait Width<const N: usize> {
        fn width(&self) -> usize;
    }

    impl<const N: usize> Width<N> for Row<N> {
        fn width(&self) -> usize {
            N
        }
    }
}

#[test]
fn a_const_parameter_may_be_bound_by_the_entry() {
    assert_eq!(Array([1, 2, 3]).count(), 3);
}

#[test]
fn a_const_parameter_may_come_from_the_trait() {
    use declared::{Row, Width};
    assert_eq!(Row([1, 2]).width(), 2);
}

/// A const pinned by an instantiation annotation, beside one that names it.
mod pinned {
    use closed_trait::sealed;

    pub struct Row3;
    pub struct Row<const N: usize>(pub [u8; N]);

    #[sealed(Row3: Width<3>, Row<N>)]
    pub trait Width<const N: usize> {
        fn width(&self) -> usize;
    }

    impl Width<3> for Row3 {
        fn width(&self) -> usize {
            3
        }
    }

    impl<const N: usize> Width<N> for Row<N> {
        fn width(&self) -> usize {
            N
        }
    }
}

#[test]
fn a_const_may_be_pinned_by_an_annotation() {
    use pinned::{Row, Row3, Width};

    assert_eq!(Row3.width(), 3);
    assert_eq!(Row([1, 2]).width(), 2);
}
