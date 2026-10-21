use crate::select::{Expression, Name, Select, SortKey};
/// A retrieval query with final sorting and execution-option syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadQuery {
    /// Retrieval body.
    pub body: ReadBody,
    /// Final ORDER BY, including for UNION.
    pub order_by: Vec<SortKey>,
    /// WITH OWNERACCESS OPTION flag.
    pub owner_access: bool,
}
/// Retrieval composition, preserving UNION branch order and duplicate modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadBody {
    /// A SELECT body, without final ordering.
    Select(Box<Select>),
    /// Stored-query reference, as documented for UNION operands.
    Reference(Name),
    /// TABLE name, as documented for UNION operands.
    Table(Name),
    /// A sequence of UNION operations; ALL is retained separately for each arm.
    Union {
        /// First operand.
        first: Box<ReadBody>,
        /// Following operands in written order.
        rest: Vec<UnionArm>,
    },
}
/// A following UNION operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionArm {
    /// UNION ALL when true; UNION otherwise.
    pub all: bool,
    /// Retrieval operand.
    pub body: ReadBody,
}
/// Access crosstab syntax, without aggregate or pivot evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crosstab {
    /// Aggregate expression following TRANSFORM.
    pub transform: Expression,
    /// Optional aggregate alias, distinct from pivot-generated output headings.
    pub transform_alias: Option<Name>,
    /// Row headings, sources, filters and grouping.
    pub select: Select,
    /// Column heading expression following PIVOT.
    pub pivot: Expression,
    /// Optional fixed headings, distinct from an IN membership predicate.
    pub headings: Option<Vec<Expression>>,
}
