//! The visual query model is input data; runtime binding still qualifies it.
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Describe<'a> {
    document_revision: u64,
    table: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Import<'a> {
    document_revision: u64,
    sql: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Rebuild {
    document_revision: u64,
    design_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Build<'a> {
    document_revision: u64,
    #[serde(borrow)]
    draft: Draft<'a>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Draft<'a> {
    #[serde(borrow)]
    sources: Vec<Source<'a>>,
    #[serde(borrow)]
    joins: Vec<Join<'a>>,
    #[serde(borrow)]
    columns: Vec<Column<'a>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Source<'a> {
    id: u64,
    table: &'a str,
    alias: Option<&'a str>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Join<'a> {
    #[serde(borrow)]
    left: Field<'a>,
    #[serde(borrow)]
    right: Field<'a>,
    kind: JoinKind,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum JoinKind {
    Inner,
    Left,
    Right,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Field<'a> {
    source_id: u64,
    column: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Column<'a> {
    #[serde(borrow)]
    expression: Operand<'a>,
    alias: Option<&'a str>,
    show: Option<bool>,
    sort: Option<Sort>,
    #[serde(borrow)]
    criteria: Option<Vec<&'a str>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Operand<'a> {
    Field { source_id: u64, column: &'a str },
    AllFields { source_id: Option<u64> },
    Expression { text: &'a str },
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Sort {
    Ascending,
    Descending,
}

pub(super) fn schema(operation: &str) -> Option<Map<String, Value>> {
    let schema = match operation {
        "describe_query_design" => schemars::schema_for!(Describe<'_>),
        "import_query_design" => schemars::schema_for!(Import<'_>),
        "build_query_design" => schemars::schema_for!(Build<'_>),
        "rebuild_query_design" => schemars::schema_for!(Rebuild),
        _ => return None,
    };
    match schema.to_value() {
        Value::Object(object) => Some(object),
        _ => None,
    }
}

pub(super) fn validate(operation: &str, args: &Value) -> Option<Result<(), String>> {
    Some(match operation {
        "describe_query_design" => super::check::<Describe<'_>>(args),
        "import_query_design" => super::check::<Import<'_>>(args),
        "build_query_design" => super::check::<Build<'_>>(args),
        "rebuild_query_design" => super::check::<Rebuild>(args),
        _ => return None,
    })
}
