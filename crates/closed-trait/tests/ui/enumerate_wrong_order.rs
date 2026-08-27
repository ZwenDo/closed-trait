use closed_trait::{enumerate, sealed};

pub struct Square;

#[sealed(Square)]
#[enumerate]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
