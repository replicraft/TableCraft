//! Successful carrier guarantees of existing arithmetic, without evaluating operands.
use crate::OutputType;
use tablecraft_schema::ScalarType;
use tablecraft_schema::ScalarType::{
    Boolean, Byte, Currency, DateTime, Decimal, Double, Int16, Int32, Int64, Single, Text,
};
use tablecraft_sql::select::BinaryOperator;
use tablecraft_sql::select::BinaryOperator::{
    Add, Divide, IntegerDivide, Modulo, Multiply, Power, Subtract,
};

pub(super) fn result(operator: BinaryOperator, left: OutputType, right: OutputType) -> OutputType {
    if !matches!(
        operator,
        Add | Subtract | Multiply | Divide | IntegerDivide | Modulo | Power
    ) {
        return OutputType::Dynamic;
    }
    if left == OutputType::Null || right == OutputType::Null {
        let other = if left == OutputType::Null {
            right
        } else {
            left
        };
        if let OutputType::Scalar { data_type, .. } = other
            && !numeric_operand(data_type.scalar_type())
        {
            return OutputType::Dynamic;
        }

        return OutputType::Null;
    }
    let (
        OutputType::Scalar {
            data_type: left,
            nullable: a,
        },
        OutputType::Scalar {
            data_type: right,
            nullable: b,
        },
    ) = (left, right)
    else {
        return OutputType::Dynamic;
    };
    let (left, right) = (left.scalar_type(), right.scalar_type());
    if !numeric_operand(left) || !numeric_operand(right) {
        return OutputType::Dynamic;
    }
    if operator == Add && (left == Text) != (right == Text) {
        // The same prepared plan can execute with materialized or declared
        // arithmetic. Mixed String/numeric + can then produce numeric or Text
        // results, so neither carrier is a metadata guarantee.
        return OutputType::Dynamic;
    }

    let scalar = carrier(operator, left, right);
    scalar.map_or(OutputType::Dynamic, |scalar| {
        OutputType::scalar(scalar, a || b)
    })
}

fn carrier(operator: BinaryOperator, left: ScalarType, right: ScalarType) -> Option<ScalarType> {
    if operator == Power {
        return Some(Double);
    }
    if matches!(operator, IntegerDivide | Modulo) {
        let target = rounded_target(left, right);
        // Remainders fit their converted operand width. Quotients may widen at
        // MIN / -1 except Byte; LongLong retains checked overflow instead.
        return (operator == Modulo || matches!(target, Byte | Int64)).then_some(target);
    }
    if operator == Add && left == Text && right == Text {
        return Some(Text);
    }
    if left == Decimal || right == Decimal {
        return Some(Decimal);
    }
    let date = left == DateTime || right == DateTime;
    if date {
        return match operator {
            Multiply | Divide => Some(Double),
            Subtract if left == DateTime && right == DateTime => Some(Double),
            // DATE +/- may leave the valid civil envelope and return Double.
            _ => None,
        };
    }
    let currency = left == Currency || right == Currency;
    if currency {
        return Some(match operator {
            Divide => Double,
            Multiply
                if matches!(left, Single | Double | Text)
                    || matches!(right, Single | Double | Text) =>
            {
                Double
            }
            _ => Currency,
        });
    }
    // Contextual numeric Text conversion selects Double after the special
    // Decimal/Currency/DATE and Text+Text cases above.
    if matches!(left, Double | Text) || matches!(right, Double | Text) {
        return Some(Double);
    }
    if left == Single || right == Single {
        return if small_float_operand(left) && small_float_operand(right) {
            None // Single overflow can produce Double even for division.
        } else {
            Some(Double)
        };
    }
    if operator == Divide {
        return Some(Double);
    }
    // Narrow integer arithmetic widens with materialized values. LongLong
    // deliberately raises overflow instead of selecting a wider carrier.
    (left == Int64 || right == Int64).then_some(Int64)
}

fn rounded_target(left: ScalarType, right: ScalarType) -> ScalarType {
    if left == Int64 || right == Int64 {
        Int64
    } else if matches!(left, Boolean | Int16) && fractional(right) {
        Int16
    } else if fractional(left) || fractional(right) || left == Int32 || right == Int32 {
        Int32
    } else if left == Byte && right == Byte {
        Byte
    } else {
        Int16
    }
}

fn fractional(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Single
            | ScalarType::Double
            | ScalarType::Currency
            | ScalarType::Decimal
            | ScalarType::DateTime
            | ScalarType::Text
    )
}

fn small_float_operand(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Byte | ScalarType::Boolean | ScalarType::Int16 | ScalarType::Single
    )
}

fn numeric_operand(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Boolean
            | ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int32
            | ScalarType::Int64
            | ScalarType::Single
            | ScalarType::Double
            | ScalarType::Currency
            | ScalarType::Decimal
            | ScalarType::DateTime
            | ScalarType::Text
    )
}
