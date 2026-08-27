use closed_trait::{enumerate, sealed};

pub struct Square;

#[enumerate(visitor)]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
