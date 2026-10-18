use super::FieldReference;

/// Syntax only. Integer literals are not assigned a stored Access field type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    /// Unresolved field reference (which may later bind as an implicit parameter).
    Field(FieldReference),
    /// Signed 64-bit integer syntax; no decimal or floating conversion.
    Integer(i64),
    /// Single-quoted string contents.
    Text(String),
    /// NULL literal.
    Null,
    /// TRUE or FALSE literal, without choosing an Access numeric encoding.
    Boolean(bool),
    /// Binary expression with an explicitly supported operator.
    Binary {
        /// Left operand.
        left: Box<Expression>,
        /// Operator.
        operator: BinaryOperator,
        /// Right operand.
        right: Box<Expression>,
    },
    /// IS NULL or IS NOT NULL.
    IsNull {
        /// Operand.
        expression: Box<Expression>,
        /// Whether NOT was specified.
        negated: bool,
    },
}

/// Supported comparison and logical syntax; no SQL equality implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    /// Equals.
    Equal,
    /// Not equal.
    NotEqual,
    /// Less than.
    Less,
    /// Less than or equal.
    LessOrEqual,
    /// Greater than.
    Greater,
    /// Greater than or equal.
    GreaterOrEqual,
    /// Logical AND.
    And,
    /// Logical OR.
    Or,
}
