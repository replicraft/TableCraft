//! Result descriptors for exact UNION widening, separate from branch selection.
use super::OutputType;
use tablecraft_schema::{ScalarType, TypeDescriptor};

pub(super) fn common(left: OutputType, right: OutputType) -> OutputType {
    let (left, right, nullable) = match (left, right) {
        (OutputType::Null, other) | (other, OutputType::Null) => return other.with_null(),
        (
            OutputType::Scalar {
                data_type: left,
                nullable: a,
            },
            OutputType::Scalar {
                data_type: right,
                nullable: b,
            },
        ) => (left, right, a || b),
        _ => return OutputType::Dynamic,
    };
    let descriptor = if left == right {
        left
    } else if let Some(kind) = integers(left.scalar_type(), right.scalar_type()) {
        TypeDescriptor::new(kind)
    } else {
        match (left.scalar_type(), right.scalar_type()) {
            (ScalarType::Single, ScalarType::Double) | (ScalarType::Double, ScalarType::Single) => {
                TypeDescriptor::new(ScalarType::Double)
            }
            (ScalarType::Text, ScalarType::Text) => {
                match (left.max_text_characters(), right.max_text_characters()) {
                    (Some(a), Some(b)) => {
                        if a >= b {
                            left
                        } else {
                            right
                        }
                    }
                    _ => TypeDescriptor::new(ScalarType::Text),
                }
            }
            // Different declared scales/precisions remain exact per value; no
            // rescaling or rounding is implied by an unconstrained result carrier.
            (ScalarType::Decimal, ScalarType::Decimal) => TypeDescriptor::new(ScalarType::Decimal),
            _ => return OutputType::Dynamic,
        }
    };
    OutputType::Scalar {
        data_type: descriptor,
        nullable,
    }
}

fn integers(left: ScalarType, right: ScalarType) -> Option<ScalarType> {
    let width = |kind| match kind {
        ScalarType::Byte => Some(size_of::<u8>()),
        ScalarType::Int16 => Some(size_of::<i16>()),
        ScalarType::Int32 => Some(size_of::<i32>()),
        ScalarType::Int64 => Some(size_of::<i64>()),
        _ => None,
    };
    Some(if width(left)? >= width(right)? {
        left
    } else {
        right
    })
}
