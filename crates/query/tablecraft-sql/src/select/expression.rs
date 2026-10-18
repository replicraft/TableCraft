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
    /// Exact numeric spelling outside the i64 carrier (decimal/exponent/hex/octal).
    Number(String),
    /// Date/time literal contents, retained without locale/calendar interpretation.
    DateLiteral(String),
    /// Canonical GUID literal spelling, without braces; syntactically validated.
    GuidLiteral(String),
    /// Last-insert identity reference; no connection state is read by the parser.
    Identity,
    /// A named function call; this does not resolve or evaluate the function.
    Call {
        /// Optional module/library qualifiers, in written order.
        qualifier: Vec<super::Name>,
        /// Function identifier.
        name: super::Name,
        /// Arguments in written order, including omitted optional positions.
        arguments: Vec<FunctionArgument>,
    },
    /// Prefix operator.
    Unary {
        /// Operator.
        operator: UnaryOperator,
        /// Operand.
        expression: Box<Expression>,
    },
    /// LIKE pattern syntax without choosing a wildcard mode or evaluating matches.
    Like {
        /// Tested expression.
        expression: Box<Expression>,
        /// Pattern expression.
        pattern: Box<Expression>,
        /// Whether NOT precedes LIKE.
        negated: bool,
    },
    /// Collection/property access using the Access bang operator.
    Member {
        /// Owning object or collection.
        expression: Box<Expression>,
        /// Member name.
        member: super::Name,
        /// Bang when true, dot otherwise.
        bang: bool,
    },
    /// Scalar retrieval subquery.
    Subquery(Box<crate::query::ReadQuery>),
    /// EXISTS retrieval subquery.
    Exists(Box<crate::query::ReadQuery>),
    /// IN/NOT IN retrieval subquery, distinct from a literal/expression list.
    InQuery {
        /// Tested expression.
        expression: Box<Expression>,
        /// Retrieval query.
        query: Box<crate::query::ReadQuery>,
        /// NOT IN flag.
        negated: bool,
    },
    /// Comparison against ALL/ANY/SOME subquery results.
    Quantified {
        /// Left operand.
        expression: Box<Expression>,
        /// Comparison operator.
        operator: BinaryOperator,
        /// Written quantifier.
        quantifier: Quantifier,
        /// Retrieval query.
        query: Box<crate::query::ReadQuery>,
    },
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
    /// Addition (including Access text addition semantics at evaluation time).
    Add,
    /// Subtraction.
    Subtract,
    /// Multiplication.
    Multiply,
    /// Division.
    Divide,
    /// Integer division.
    IntegerDivide,
    /// Modulo.
    Modulo,
    /// Exponentiation.
    Power,
    /// Ampersand concatenation.
    Concatenate,
    /// Logical exclusive OR.
    Xor,
    /// Logical equivalence.
    Equivalence,
    /// Logical implication.
    Implication,
    /// Logical AND.
    And,
    /// Logical OR.
    Or,
}

/// Written function argument, keeping omission separate from NULL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionArgument {
    /// An expression argument.
    Expression(Expression),
    /// An asterisk argument, such as COUNT(*).
    Wildcard,
    /// An omitted optional argument between commas.
    Missing,
}

/// Access prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// Numeric identity.
    Plus,
    /// Numeric negation.
    Minus,
    /// Logical/bitwise NOT; meaning belongs to evaluation.
    Not,
}

/// Subquery comparison quantifiers, retained without evaluating results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    /// ALL.
    All,
    /// ANY.
    Any,
    /// SOME, retaining its written distinction from ANY.
    Some,
}
