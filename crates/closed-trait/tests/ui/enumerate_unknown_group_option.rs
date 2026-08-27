use closed_trait::{enumerate, sealed};

pub struct Square;

// `crate` is global, not something one enum can carry.
#[enumerate(ref(crate = "::closed_trait"))]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
