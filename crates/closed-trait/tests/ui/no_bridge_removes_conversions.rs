use closed_trait::{enumerate, sealed};

pub struct Square;

#[enumerate(no_bridge)]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {
    let owned = AnyShape::from(Square);
    // `no_bridge` left this out.
    let _ = owned.as_ref();
}
