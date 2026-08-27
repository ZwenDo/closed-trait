use closed_trait::sealed;

pub struct Square;
pub struct Triangle;

#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}
impl Shape for Triangle {}

fn main() {}
