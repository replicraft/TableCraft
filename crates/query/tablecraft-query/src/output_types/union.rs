//! UNION common carriers, separate from conditional selection and stored schemas.
use super::OutputType;
use tablecraft_schema::ScalarType::{
    Boolean, Byte, Currency, Decimal, Double, Int16, Int32, Int64, Single,
};
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
    } else if let Some(kind) = numeric(left.scalar_type(), right.scalar_type()) {
        TypeDescriptor::new(kind)
    } else {
        match (left.scalar_type(), right.scalar_type()) {
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
            _ => return OutputType::Dynamic,
        }
    };
    OutputType::Scalar {
        data_type: descriptor,
        nullable,
    }
}

fn numeric(left: ScalarType, right: ScalarType) -> Option<ScalarType> {
    let admitted = |kind| {
        matches!(
            kind,
            Boolean | Byte | Int16 | Int32 | Int64 | Single | Double | Currency | Decimal
        )
    };
    if !admitted(left) || !admitted(right) {
        return None;
    }
    if left == right {
        return Some(left);
    }
    if matches!(left, Single | Double) || matches!(right, Single | Double) {
        return Some(Double);
    }
    if left == Decimal || right == Decimal {
        return Some(Decimal);
    }
    if left == Currency || right == Currency {
        // Every Long fits Currency; the full LongLong domain needs Decimal.
        return Some(if left == Int64 || right == Int64 {
            Decimal
        } else {
            Currency
        });
    }
    let left = if left == Boolean { Int16 } else { left };
    let right = if right == Boolean { Int16 } else { right };
    integers(left, right)
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
