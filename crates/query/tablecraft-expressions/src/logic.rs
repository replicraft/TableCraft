//! Boolean/NULL truth and explicit exact-integer logical operations.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

mod integer;
pub use integer::{Logical, complement};

/// SQL truth, keeping unknown separate from false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    /// TRUE.
    True,
    /// FALSE.
    False,
    /// Unknown, represented by SQL NULL.
    Unknown,
}
impl Truth {
    /// Convert only Boolean or NULL; numeric bit patterns are not booleans here.
    pub fn from_value(value: &Value) -> Result<Self, EvalError> {
        match value {
            Value::Boolean(true) => Ok(Self::True),
            Value::Boolean(false) => Ok(Self::False),
            Value::Null => Ok(Self::Unknown),
            _ => Err(EvalError::TypeMismatch),
        }
    }
    /// Interpret a final predicate without discarding intermediate integer bits.
    /// Numeric and legacy DATE values use exact zero/nonzero Boolean conversion;
    /// NULL remains unknown. Text needs a separate explicit host conversion.
    pub fn from_predicate(value: &Value) -> Result<Self, EvalError> {
        if matches!(value, Value::Null) {
            return Ok(Self::Unknown);
        }
        let value = Conversion::Boolean.apply(value)?;
        Self::from_value(&value)
    }

    /// Materialize as Boolean or NULL.
    pub const fn into_value(self) -> Value {
        match self {
            Self::True => Value::Boolean(true),
            Self::False => Value::Boolean(false),
            Self::Unknown => Value::Null,
        }
    }
    /// A WHERE-style predicate selects only TRUE.
    pub const fn is_true(self) -> bool {
        matches!(self, Self::True)
    }
    /// Logical NOT, retaining unknown.
    pub const fn negate(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
    /// Logical AND with FALSE dominating unknown.
    pub const fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }
    /// Logical OR with TRUE dominating unknown.
    pub const fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }
    /// Exclusive OR; unknown propagates.
    pub const fn xor(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::True, Self::False) | (Self::False, Self::True) => Self::True,
            _ => Self::False,
        }
    }
    /// Logical equivalence.
    pub const fn equivalent(self, other: Self) -> Self {
        self.xor(other).negate()
    }
    /// Logical implication; FALSE implies unknown is TRUE.
    pub const fn implies(self, other: Self) -> Self {
        self.negate().or(other)
    }
}

#[cfg(test)]
mod tests {
    use super::{Truth::*, *};
    #[test]
    fn exhaustive_three_valued_tables_and_predicate_selection() {
        let truths = [True, False, Unknown];
        let and = [
            [True, False, Unknown],
            [False, False, False],
            [Unknown, False, Unknown],
        ];
        let or = [
            [True, True, True],
            [True, False, Unknown],
            [True, Unknown, Unknown],
        ];
        let xor = [
            [False, True, Unknown],
            [True, False, Unknown],
            [Unknown, Unknown, Unknown],
        ];
        let imp = [
            [True, False, Unknown],
            [True, True, True],
            [True, Unknown, Unknown],
        ];
        for (i, left) in truths.into_iter().enumerate() {
            assert_eq!(left.negate().negate(), left);
            assert_eq!(Truth::from_value(&left.into_value()).unwrap(), left);
            assert_eq!(left.is_true(), i == 0);
            for (j, right) in truths.into_iter().enumerate() {
                assert_eq!(left.and(right), and[i][j]);
                assert_eq!(left.or(right), or[i][j]);
                assert_eq!(left.xor(right), xor[i][j]);
                assert_eq!(left.equivalent(right), xor[i][j].negate());
                assert_eq!(left.implies(right), imp[i][j]);
            }
        }
        assert_eq!(
            Truth::from_value(&Value::Int16(-1)),
            Err(EvalError::TypeMismatch)
        );
    }
}
