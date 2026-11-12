use tablecraft_schema::ScalarType;
use tablecraft_sql::query::Parameter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueryDesignDraft {
    pub totals: bool,
    pub parameters: Vec<Parameter>,
    pub sources: Vec<DesignSource>,
    pub joins: Vec<DesignJoin>,
    pub columns: Vec<DesignColumn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignSource {
    pub id: u64,
    pub table: String,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignField {
    pub source_id: u64,
    pub column: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesignExpression {
    Field(DesignField),
    AllFields(Option<u64>),
    Expression(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesignJoinKind {
    Inner,
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignJoin {
    pub left: DesignField,
    pub right: DesignField,
    pub kind: DesignJoinKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesignSort {
    Ascending,
    Descending,
}

/// Explicit query-design Total row choice, not a datasheet footer operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DesignTotal {
    #[default]
    GroupBy,
    Sum,
    Avg,
    Min,
    Max,
    Count,
    First,
    Last,
    Expression,
    Where,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignColumn {
    pub total: DesignTotal,
    pub expression: DesignExpression,
    pub alias: Option<String>,
    pub show: bool,
    pub sort: Option<DesignSort>,
    /// Same row across columns is AND; distinct nonempty rows are OR.
    pub criteria: Vec<String>,
}

impl DesignColumn {
    pub fn field(source_id: u64, column: String) -> Self {
        Self {
            total: DesignTotal::GroupBy,
            expression: DesignExpression::Field(DesignField { source_id, column }),
            alias: None,
            show: true,
            sort: None,
            criteria: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignFieldMetadata {
    pub name: String,
    pub scalar_type: Option<ScalarType>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignTableMetadata {
    pub table: String,
    pub fields: Vec<DesignFieldMetadata>,
}

pub struct BuiltQueryDesign {
    pub draft: QueryDesignDraft,
    pub sql: String,
}
