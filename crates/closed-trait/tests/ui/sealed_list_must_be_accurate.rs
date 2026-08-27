use closed_trait::sealed;

pub struct Square;
pub struct Triangle;

#[sealed(Square, Triangle)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
