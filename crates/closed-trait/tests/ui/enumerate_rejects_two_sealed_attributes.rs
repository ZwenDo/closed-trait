use closed_trait::{enumerate, sealed};

pub struct Square;
pub struct Circle;

// Only the first list would be read, and the second would seal the trait a
// second time.
#[enumerate]
#[sealed(Square)]
#[sealed(Circle)]
pub trait Shape {}

impl Shape for Square {}
impl Shape for Circle {}

fn main() {}
