//! Wire shapes for retained filters, their inert definitions and totals.
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Field<'a> {
    document_revision: u64,
    table: &'a str,
    column: &'a str,
    filter_id: Option<u64>,
    enabled: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Apply<'a> {
    document_revision: u64,
    table: &'a str,
    column: &'a str,
    filter_id: Option<u64>,
    enabled: Option<bool>,
    operator: &'a str,
    #[serde(borrow)]
    inputs: Vec<&'a str>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Selection<'a> {
    document_revision: u64,
    table: &'a str,
    column: &'a str,
    parent_ordinal: u32,
    comparison: Comparison,
    filter_id: Option<u64>,
    enabled: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Comparison {
    Equal,
    NotEqual,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Page {
    document_revision: u64,
    filter_id: u64,
    offset: u32,
    enabled: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Clear {
    document_revision: u64,
    filter_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Reapply {
    document_revision: u64,
    definition_id: u64,
    offset: u32,
    enabled: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Sort<'a> {
    document_revision: u64,
    table: &'a str,
    filter_id: Option<u64>,
    enabled: Option<bool>,
    #[serde(borrow)]
    sort: Option<SortSpec<'a>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SortSpec<'a> {
    column: &'a str,
    descending: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TotalsTarget<'a> {
    document_revision: u64,
    table: &'a str,
    filter_id: Option<u64>,
    enabled: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Totals<'a> {
    document_revision: u64,
    table: &'a str,
    filter_id: Option<u64>,
    enabled: Option<bool>,
    #[serde(borrow)]
    specs: Vec<TotalSpec<'a>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TotalSpec<'a> {
    column: &'a str,
    aggregate: Aggregate,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Aggregate {
    Count,
    Sum,
    Average,
    Minimum,
    Maximum,
    StandardDeviation,
    Variance,
}

pub(super) fn schema(operation: &str) -> Option<Map<String, Value>> {
    let schema = match operation {
        "describe_field_filter" => schemars::schema_for!(Field<'_>),
        "apply_field_filter" => schemars::schema_for!(Apply<'_>),
        "filter_selection" => schemars::schema_for!(Selection<'_>),
        "filter_page" => schemars::schema_for!(Page),
        "clear_filter" => schemars::schema_for!(Clear),
        "reapply_filter" => schemars::schema_for!(Reapply),
        "sort_view" => schemars::schema_for!(Sort<'_>),
        "describe_totals" => schemars::schema_for!(TotalsTarget<'_>),
        "calculate_totals" => schemars::schema_for!(Totals<'_>),
        _ => return None,
    };
    match schema.to_value() {
        Value::Object(object) => Some(object),
        _ => None,
    }
}

pub(super) fn validate(operation: &str, args: &Value) -> Option<Result<(), String>> {
    Some(match operation {
        "describe_field_filter" => super::check::<Field<'_>>(args),
        "apply_field_filter" => super::check::<Apply<'_>>(args),
        "filter_selection" => super::check::<Selection<'_>>(args),
        "filter_page" => super::check::<Page>(args),
        "clear_filter" => super::check::<Clear>(args),
        "reapply_filter" => super::check::<Reapply>(args),
        "sort_view" => super::check::<Sort<'_>>(args),
        "describe_totals" => super::check::<TotalsTarget<'_>>(args),
        "calculate_totals" => super::check::<Totals<'_>>(args),
        _ => return None,
    })
}
