use closed_trait::{enumerate, sealed};

pub struct Dot(pub i32);

pub struct Tail {
    pub head: f64,
    pub rest: [i32],
}

#[enumerate(match_any)]
#[sealed(Dot, Tail)]
pub trait Shape {
    fn area(&self) -> f64;
}

impl Shape for Dot {
    fn area(&self) -> f64 {
        1.0
    }
}

impl Shape for Tail {
    fn area(&self) -> f64 {
        self.head
    }
}

fn main() {}
