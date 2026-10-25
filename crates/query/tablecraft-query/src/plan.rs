pub(crate) mod declarations;
mod profiled_type;
pub(crate) use profiled_type::{ProfiledType, ResultProfile};
mod domain;
pub(crate) use domain::DomainFunction;
mod statistics;
use crate::{OutputType, source::TextPolicy};
pub(crate) use statistics::Statistic;
use std::ops::{Deref, Range};
use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::arrays::BoundFunction;
use tablecraft_expressions::functions::{
    conditional::ConditionalFunction,
    conversions::Conversion,
    dates::{DateConstructor, DateFunction, clock::ClockFunction, parsing::DateConversion},
    financial::{FinancialFunction, cashflows::CashFlowFunction},
    inspection::{InspectionFunction, types::ValueInspectionFunction},
    math::MathFunction,
    text::TextFunction,
};
use tablecraft_expressions::numeric::OperandDeclaration;
use tablecraft_schema::Column;
use tablecraft_sql::select::{BinaryOperator, Expression, JoinKind, Quantifier, UnaryOperator};

pub(crate) mod identity;
mod parent;
pub(crate) mod text_policy;
pub(crate) use parent::{
    ParentFilter, ParentLocation, ParentMember, ParentPredicate, ParentProduct,
};

pub(crate) enum PreparedBody {
    Read(ReadPlan),
    Crosstab(CrosstabPlan),
}
pub(crate) struct CrosstabPlan {
    pub output_types: Option<Vec<OutputType>>,
    pub materialized_types: Option<Vec<OutputType>>,
    pub declared_types: Option<Vec<OutputType>>,
    pub materialized: Option<usize>,
    pub read: Box<ReadPlan>,
    pub pivot: Expr,
    pub headings: Option<Vec<Expr>>,
    pub text_policies: Option<Vec<TextPolicy>>,
}
pub(crate) struct Slot {
    pub output_type: ProfiledType,
    pub declaration: Option<OperandDeclaration>,
    pub text_policy: TextPolicy,
    // Unavailable slots retain names for binding but occupy no runtime row cell.
    pub available: bool,
    // Virtual .Value slot: excluded from ordinary name/wildcard resolution.
    pub member: Option<MemberSlot>,
    pub qualifier: Option<String>,
    pub name: String,
}
#[derive(Clone)]
pub(crate) enum MemberSlot {
    Value,
    Named(String),
}

impl Deref for MemberSlot {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Self::Value => "Value",
            Self::Named(name) => name,
        }
    }
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
        namespace: crate::external_sources::SourceNamespace,
        name: String,
        columns: Vec<Column>,
        unavailable: Vec<tablecraft_schema::SchemaName>,
        text_collations: Vec<Option<crate::source::TextCollationId>>,
        projection: Vec<usize>,
        multivalues: Vec<Column>,
        multivalue_text_collations: Vec<Option<crate::source::TextCollationId>>,
        member_text_collations: Vec<Option<crate::source::TextCollationId>>,
        members: Vec<tablecraft_schema::CollectionMember>,
        expansion: Vec<usize>,
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
    pub owner: crate::external_sources::SourceNamespace,
    pub output_types: Vec<OutputType>,
    pub materialized_types: Vec<OutputType>,
    pub declared_types: Vec<OutputType>,
    pub declarations: Vec<Option<OperandDeclaration>>,
    pub text_policies: Vec<TextPolicy>,
    pub names: Vec<String>,
    pub node: ReadNode,
    pub sort: Vec<(Expr, bool)>,
    pub distinct: bool,
    pub top: Option<(u32, bool)>,
}
pub(crate) enum ReadNode {
    Select(Box<SelectPlan>),
    // The inner plan owns grouping, sorting and TOP. This wrapper supplies only
    // the fixed schema and has no additional read modifiers of its own.
    Crosstab(Box<CrosstabPlan>),
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
    pub parent: Option<ParentFilter>,
}

#[derive(Clone, Copy)]
pub(crate) struct ProjectionReference {
    pub index: usize,
    pub output_type: ProfiledType,
    pub declaration: Option<OperandDeclaration>,
    pub text_policy: TextPolicy,
}

pub(crate) enum Expr {
    DomainCall(DomainFunction, Vec<Expr>),
    Literal(Expression),
    // Captured numeric carrier; one source literal remains one plan leaf.
    NumericLiteral(Value),
    // Written spelling retains structural identity; the checked date is lexical.
    DateLiteral(String, LegacyDateTime),
    Field {
        text_policy: TextPolicy,
        depth: usize,
        index: usize,
    },
    Output(usize, TextPolicy),
    Projected(ProjectionReference),
    // Current membership item in this query scope or a correlated outer scope.
    CollectionItem {
        text_policy: TextPolicy,
        depth: usize,
        index: usize,
        width: usize,
    },
    Parameter(usize),
    Unary(UnaryOperator, Box<Expr>),
    Binary(BinaryOperator, Box<Expr>, Box<Expr>),
    IsNull(Box<Expr>, bool),
    // WHERE-only container test; lifted to a typed parent predicate before execution.
    CollectionEmpty(Box<Expr>, bool),
    // Resolved key in the current parent-membership frame; never a stored scalar.
    ParentPresence {
        collection: usize,
        negated: bool,
    },
    IIf(Box<Expr>, Box<Expr>, Box<Expr>),
    DateAdd(Box<Expr>, Box<Expr>, Box<Expr>),
    DateDiff(Box<Expr>, Box<Expr>, Box<Expr>, [Option<Box<Expr>>; 2]),
    DatePart(Box<Expr>, Box<Expr>, [Option<Box<Expr>>; 2]),
    Weekday(Box<Expr>, Option<Box<Expr>>),
    Nz(Box<Expr>, Option<Box<Expr>>),
    TextCall(TextFunction, Vec<Expr>),
    MathCall(MathFunction, Vec<Expr>),
    FinancialCall(FinancialFunction, Vec<Expr>),
    // Flattened written operands with an explicit cashflow-element range.
    CashFlowCall(CashFlowFunction, Vec<Expr>, Range<usize>),
    ConstructorCall(DateConstructor, Vec<Expr>),
    ConditionalCall(ConditionalFunction, Vec<Expr>),
    DateCall(DateFunction, Box<Expr>),
    Convert(Conversion, Box<Expr>),
    CVar(Box<Expr>),
    CvErr(Box<Expr>),
    Eval(Box<Expr>),
    GuidFromString(Box<Expr>),
    StringFromGuid(Box<Expr>),
    ErrorMessage(Box<Expr>),
    ErrorMessageString(Box<Expr>),
    Inspect(InspectionFunction, Box<Expr>),
    ValueInspect(ValueInspectionFunction, Box<Expr>),
    Array(Vec<Expr>),
    ArrayBound(BoundFunction, Box<Expr>, Box<Expr>),
    DateConvert(DateConversion, Box<Expr>),
    Clock(ClockFunction),
    CurrentUser,
    Like(Box<Expr>, Box<Expr>, bool),
    Between(Box<Expr>, Box<Expr>, Box<Expr>, bool),
    InList(Box<Expr>, Vec<Expr>, bool),
    Aggregate(Aggregate, Option<Box<Expr>>),
    Subquery(Box<ReadPlan>),
    Exists(Box<ReadPlan>),
    InQuery(Box<Expr>, Box<ReadPlan>, bool),
    Quantified(Box<Expr>, BinaryOperator, Quantifier, Box<ReadPlan>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aggregate {
    Statistic(Statistic),
    Count,
    Sum,
    Average,
    Minimum,
    Maximum,
    First,
    Last,
}

impl ReadPlan {
    pub(crate) fn active_types(&self, declared: bool) -> &[OutputType] {
        self.profile_types(if declared {
            ResultProfile::Declared
        } else {
            ResultProfile::Materialized
        })
    }

    pub(crate) fn profile_types(&self, profile: ResultProfile) -> &[OutputType] {
        match profile {
            ResultProfile::Materialized => &self.materialized_types,
            ResultProfile::Declared => &self.declared_types,
        }
    }
}
