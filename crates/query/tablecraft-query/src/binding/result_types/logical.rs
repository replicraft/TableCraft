//! Successful carriers of the existing logical kernels; no value coercion or scans.
use crate::OutputType;
use tablecraft_schema::ScalarType;
use tablecraft_schema::ScalarType::{Boolean, Byte, Int16, Int32, Int64};
use tablecraft_sql::select::BinaryOperator;

pub(super) fn complement(input: OutputType) -> OutputType {
    match input {
        OutputType::Null => OutputType::Null,
        OutputType::Scalar {
            data_type,
            nullable,
        } => {
            let kind = data_type.scalar_type();
            let result = match kind {
                Boolean | Byte | Int16 | Int32 | Int64 => kind,
                _ if rounded(kind) => Int32,
                _ => return OutputType::Dynamic,
            };
            OutputType::scalar(result, nullable)
        }
        OutputType::Multivalue { .. } | OutputType::Dynamic => OutputType::Dynamic,
    }
}

pub(super) fn binary(operator: BinaryOperator, left: OutputType, right: OutputType) -> OutputType {
    let (Some(left), Some(right)) = (Operand::new(left), Operand::new(right)) else {
        return OutputType::Dynamic;
    };
    let mut result: Option<OutputType> = None;
    for left_kind in [left.kind, None].into_iter().take(left.variants()) {
        for right_kind in [right.kind, None].into_iter().take(right.variants()) {
            let branch = concrete(operator, left_kind, right_kind);
            result = Some(match result {
                None => branch,
                Some(previous) => previous.union(branch),
            });
        }
    }

    result.unwrap_or(OutputType::Dynamic)
}

fn concrete(
    operator: BinaryOperator,
    left: Option<ScalarType>,
    right: Option<ScalarType>,
) -> OutputType {
    let null = left.is_none() || right.is_none();
    if left.is_none() && right.is_none()
        || null && matches!(operator, BinaryOperator::Xor | BinaryOperator::Equivalence)
    {
        return OutputType::Null;
    }

    let kind = if left.is_some_and(rounded) || right.is_some_and(rounded) {
        // Select from original carriers, before contextual Text conversion.
        if left == Some(Int64) || right == Some(Int64) {
            Int64
        } else {
            Int32
        }
    } else if left.is_none_or(|kind| kind == Boolean) && right.is_none_or(|kind| kind == Boolean) {
        Boolean
    } else if left == Some(Int64) || right == Some(Int64) {
        Int64
    } else if left == Some(Int32) || right == Some(Int32) {
        Int32
    } else if left.is_none_or(|kind| kind == Byte) && right.is_none_or(|kind| kind == Byte) {
        Byte
    } else {
        Int16
    };

    OutputType::scalar(kind, null)
}

struct Operand {
    kind: Option<ScalarType>,
    nullable: bool,
}

impl Operand {
    fn new(input: OutputType) -> Option<Self> {
        match input {
            OutputType::Null => Some(Self {
                kind: None,
                nullable: true,
            }),
            OutputType::Scalar {
                data_type,
                nullable,
            } => {
                let kind = data_type.scalar_type();
                (matches!(kind, Boolean | Byte | Int16 | Int32 | Int64) || rounded(kind)).then_some(
                    Self {
                        kind: Some(kind),
                        nullable,
                    },
                )
            }
            OutputType::Multivalue { .. } | OutputType::Dynamic => None,
        }
    }

    fn variants(&self) -> usize {
        if self.kind.is_some() && self.nullable {
            2
        } else {
            1
        }
    }
}

fn rounded(kind: ScalarType) -> bool {
    matches!(
        kind,
        ScalarType::Single
            | ScalarType::Double
            | ScalarType::Currency
            | ScalarType::Decimal
            | ScalarType::DateTime
            | ScalarType::Text
    )
}
