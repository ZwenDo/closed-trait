use closed_trait::sealed;

pub struct Slice<'a>(pub &'a str);

// `'b` is neither the trait's nor bound, so what it means is ambiguous.
#[sealed(Slice<'b>)]
pub trait Text<'a> {
    fn text(&self) -> &str;
}

impl<'a, 'b> Text<'a> for Slice<'b> {
    fn text(&self) -> &str {
        self.0
    }
}

fn main() {}
