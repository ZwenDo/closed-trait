//! A sealed trait as a *library* publishes one, so that this crate's own
//! integration tests reach the macro across a crate boundary.

pub struct Square {
    pub side: i32,
}
pub struct Circle {
    pub radius: i32,
}

pub mod shapes {
    use closed_trait::{enumerate, sealed};

    use crate::{Circle, Square};

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
}
