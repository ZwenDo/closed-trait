use closed_trait::{enumerate, sealed};

// The supertraits have to be looked up under the path `crate` gives, never `::closed_trait`.
// The facade *is* in scope here, so this case only fails if the option is actually honoured —
// were it ignored, the generated code would resolve against `::closed_trait` and compile.
mod elsewhere {}

pub struct Square;

#[enumerate(crate = "crate::elsewhere")]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
