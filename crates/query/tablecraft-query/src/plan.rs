use tablecraft_schema::Column;
use tablecraft_sql::select::{BinaryOperator, Expression, JoinKind, Quantifier, UnaryOperator};

pub(crate) struct Slot {
    pub qualifier: Option<String>,
    pub name: String,
}
pub(crate) struct Relation {
    pub slots: Vec<Slot>,
    pub node: RelationNode,
}
pub(crate) enum RelationNode {
    Unit,
    Table {
        name: String,
        columns: Vec<Column>,
    },
    Cross(Box<Relation>, Box<Relation>),
    Join {
        left: Box<Relation>,
        right: Box<Relation>,
        kind: JoinKind,
        condition: Expr,
    },
    Derived(Box<ReadPlan>),
}
pub(crate) struct ReadPlan {
    pub names: Vec<String>,
    pub node: ReadNode,
    pub sort: Vec<(Expr, bool)>,
    pub distinct: bool,
    pub top: Option<(u32, bool)>,
}
pub(crate) enum ReadNode {
    Select(Box<SelectPlan>),
    Union {
        first: Box<ReadPlan>,
        rest: Vec<(bool, ReadPlan)>,
    },
}
pub(crate) struct SelectPlan {
    pub source: Relation,
    pub projection: Vec<Expr>,
    pub filter: Option<Expr>,
    pub group: Vec<Expr>,
    pub having: Option<Expr>,
    pub aggregate: bool,
}
pub(crate) enum Expr {
    Literal(Expression),
    Field { depth: usize, index: usize },
    Output(usize),
    Parameter(usize),
    Unary(UnaryOperator, Box<Expr>),
    Binary(BinaryOperator, Box<Expr>, Box<Expr>),
    IsNull(Box<Expr>, bool),
    Like(Box<Expr>, Box<Expr>, bool),
    Between(Box<Expr>, Box<Expr>, Box<Expr>, bool),
    InList(Box<Expr>, Vec<Expr>, bool),
    Aggregate(Aggregate, Option<Box<Expr>>),
    Subquery(Box<ReadPlan>),
    Exists(Box<ReadPlan>),
    InQuery(Box<Expr>, Box<ReadPlan>, bool),
    Quantified(Box<Expr>, BinaryOperator, Quantifier, Box<ReadPlan>),
}
#[derive(Clone, Copy)]
pub(crate) enum Aggregate {
    Count,
    Sum,
    Average,
    Minimum,
    Maximum,
}
