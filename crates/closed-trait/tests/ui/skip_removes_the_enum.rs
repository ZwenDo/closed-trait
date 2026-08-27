use closed_trait::{enumerate, sealed};

pub struct Square;

#[enumerate(ref(skip))]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {
    // `ref(skip)` means there is no such type, and no `as_ref` to reach it.
    let _: AnyShapeRef<'_> = AnyShape::from(Square).as_ref();
}
