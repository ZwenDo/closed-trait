//! Variants are named after the type's last path segment, which is not always
//! upper camel case.

use closed_trait::{enumerate, sealed};

/// A primitive gives a lowercase variant. The generated enums allow it, since
/// the caller never chose that name — they wrote a type.
///
/// No `match_any`: every entry pins an argument, which rules the macro out.
#[enumerate]
#[sealed(i32: Value<i32>, f64: Value<f64>)]
pub trait Value<T> {}

impl Value<i32> for i32 {}
impl Value<f64> for f64 {}

#[test]
fn a_primitive_names_its_variant() {
    let owned: AnyValue = AnyValue::i32(6);
    assert!(matches!(owned, AnyValue::i32(6)));

    // the borrowing enums carry the same names, and the same allowance
    assert!(matches!(owned.as_ref(), AnyValueRef::i32(6)));
}

/// `as Name` is the way out for anyone who would rather have `I32`.
mod renamed {
    use closed_trait::{enumerate, sealed};

    #[enumerate]
    #[sealed(i32 as I32: Count<i32>, u8 as U8: Count<u8>)]
    pub trait Count<T> {}

    impl Count<i32> for i32 {}
    impl Count<u8> for u8 {}
}

#[test]
fn an_alias_gives_a_conventional_name() {
    use renamed::AnyCount;

    assert!(matches!(AnyCount::I32(6), AnyCount::I32(_)));
}
