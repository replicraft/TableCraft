use tablecraft_expressions::functions::{
    conditional::ConditionalFunction, conversions::Conversion, dates::DateFunction,
    math::MathFunction, text::TextFunction,
};
use tablecraft_schema::Column;
use tablecraft_sql::select::{BinaryOperator, Expression, JoinKind, Quantifier, UnaryOperator};

pub(crate) enum PreparedBody {
    Read(ReadPlan),
    Crosstab(CrosstabPlan),
}
pub(crate) struct CrosstabPlan {
    pub read: ReadPlan,
    pub pivot: Expr,
    pub headings: Option<Vec<Expr>>,
}
pub(crate) struct Slot {
    // Unavailable slots retain names for binding but occupy no runtime row cell.
    pub available: bool,
    pub qualifier: Option<String>,
    pub name: String,
}
pub(crate) struct Relation {
    pub slots: Vec<Slot>,
    pub node: RelationNode,
}
impl Relation {
    pub(crate) fn row_width(&self) -> usize {
        self.slots.iter().filter(|slot| slot.available).count()
    }
}
pub(crate) enum RelationNode {
    Unit,
    Table {
        name: String,
        columns: Vec<Column>,
        unavailable: Vec<tablecraft_schema::SchemaName>,
        projection: Vec<usize>,
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
    pub record_keys: Vec<Expr>,
}
pub(crate) enum Expr {
    Literal(Expression),
    Field { depth: usize, index: usize },
    Output(usize),
    Parameter(usize),
    Unary(UnaryOperator, Box<Expr>),
    Binary(BinaryOperator, Box<Expr>, Box<Expr>),
    IsNull(Box<Expr>, bool),
    IIf(Box<Expr>, Box<Expr>, Box<Expr>),
    DateAdd(Box<Expr>, Box<Expr>, Box<Expr>),
    DateDiff(Box<Expr>, Box<Expr>, Box<Expr>, [Option<Box<Expr>>; 2]),
    DatePart(Box<Expr>, Box<Expr>, [Option<Box<Expr>>; 2]),
    Nz(Box<Expr>, Option<Box<Expr>>),
    TextCall(TextFunction, Vec<Expr>),
    MathCall(MathFunction, Vec<Expr>),
    ConditionalCall(ConditionalFunction, Vec<Expr>),
    DateCall(DateFunction, Box<Expr>),
    Convert(Conversion, Box<Expr>),
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
    First,
    Last,
}
