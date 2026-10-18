use super::FieldReference;

/// Syntax only. Integer literals are not assigned a stored Access field type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    /// Unresolved field reference (which may later bind as an implicit parameter).
    Field(FieldReference),
    /// Signed 64-bit integer syntax; no decimal or floating conversion.
    Integer(i64),
    /// Single- or double-quoted string contents, without the source delimiters.
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
    /// BETWEEN or NOT BETWEEN, retaining both bounds in their written order.
    /// No bound sorting, comparison rewrite or evaluation is performed.
    Between {
        /// Expression being tested.
        expression: Box<Expression>,
        /// First written bound; not necessarily the numerically smaller bound.
        first_bound: Box<Expression>,
        /// Second written bound; not necessarily the numerically larger bound.
        second_bound: Box<Expression>,
        /// Whether NOT directly precedes BETWEEN.
        negated: bool,
    },
    /// IN or NOT IN with a non-empty list in its original order.
    /// Duplicates and NULL entries are retained; no membership is evaluated.
    InList {
        /// Expression being tested.
        expression: Box<Expression>,
        /// Ordered list of expressions, without sorting or deduplication.
        values: Vec<Expression>,
        /// Whether NOT directly precedes IN.
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
