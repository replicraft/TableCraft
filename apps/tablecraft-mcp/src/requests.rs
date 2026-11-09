//! Wire types define both published schemas and strict borrowed input validation.
#![allow(
    dead_code,
    reason = "Request fields are deserialized to validate wire input; runtime owns semantic interpretation"
)]

mod filtering;
mod query_design;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Open<'a> {
    path: &'a str,
    password: Option<&'a str>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PathRequest<'a> {
    path: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Revision {
    document_revision: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Table<'a> {
    document_revision: u64,
    table: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Page<'a> {
    document_revision: u64,
    table: &'a str,
    offset: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Cell<'a> {
    document_revision: u64,
    table: &'a str,
    column: &'a str,
    parent_ordinal: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Row<'a> {
    document_revision: u64,
    table: &'a str,
    parent_ordinal: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Query<'a> {
    document_revision: u64,
    sql: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct QueryRun<'a> {
    document_revision: u64,
    query_id: u64,
    #[serde(borrow)]
    parameters: Option<Vec<Parameter<'a>>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Saved<'a> {
    document_revision: u64,
    object_id: &'a str,
    name: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SavedRun<'a> {
    document_revision: u64,
    object_id: &'a str,
    name: &'a str,
    #[serde(borrow)]
    parameters: Option<Vec<Parameter<'a>>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Edit<'a> {
    document_revision: u64,
    draft_id: u64,
    text: &'a str,
    is_null: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Insert<'a> {
    document_revision: u64,
    draft_id: u64,
    #[serde(borrow)]
    inputs: Vec<InsertValue<'a>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TableRename<'a> {
    document_revision: u64,
    table: &'a str,
    new_name: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TableCreation<'a> {
    document_revision: u64,
    table: &'a str,
    initial_autonumber: Option<bool>,
    add_incrementing_primary_key: Option<bool>,
    #[serde(borrow)]
    fields: Vec<Field<'a>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FieldAddition<'a> {
    document_revision: u64,
    table: &'a str,
    column: &'a str,
    kind: AdditionKind,
    max_utf16_units: Option<u8>,
    unicode_compression: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FieldTarget<'a> {
    document_revision: u64,
    table: &'a str,
    column: &'a str,
    native_number: usize,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FieldRename<'a> {
    document_revision: u64,
    table: &'a str,
    column: &'a str,
    native_number: usize,
    new_name: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct QuerySql<'a> {
    document_revision: u64,
    object_id: &'a str,
    name: &'a str,
    original_sql: &'a str,
    proposed_sql: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Expression<'a> {
    document_revision: u64,
    object_id: &'a str,
    name: &'a str,
    expected: &'a str,
    replacement: &'a str,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Apply {
    review_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Save {
    save_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Creation {
    creation_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Parameter<'a> {
    name: &'a str,
    kind: &'a str,
    text: &'a str,
    is_null: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InsertValue<'a> {
    omitted: Option<bool>,
    text: Option<&'a str>,
    is_null: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Field<'a> {
    name: &'a str,
    kind: FieldKind,
    max_utf16_units: Option<u16>,
    required: Option<bool>,
    allow_zero_length: Option<bool>,
    default_literal: Option<&'a str>,
    default_mode: Option<DefaultMode>,
    description: Option<&'a str>,
    number_size: Option<NumberSize>,
    precision: Option<u8>,
    scale: Option<u8>,
    primary: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum AdditionKind {
    Number,
    ShortText,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum DefaultMode {
    TypeDefault,
    Absent,
    Null,
    Literal,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum FieldKind {
    ShortText,
    Number,
    LargeInteger,
    YesNo,
    Currency,
    DateTime,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum NumberSize {
    Byte,
    Integer,
    LongInteger,
    Single,
    Double,
    Decimal,
    ReplicationId,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Find<'a> {
    document_revision: u64,
    table: &'a str,
    pattern: &'a str,
    matching: Match,
    direction: Direction,
    column: Option<&'a str>,
    match_case: Option<bool>,
    filter_id: Option<u64>,
    enabled: Option<bool>,
    formatted: Option<bool>,
    #[serde(borrow)]
    format_options: Option<FormatOptions<'a>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FormatOptions<'a> {
    system_locale: &'a str,
    date: DateFormat,
    time: TimeFormat,
    currency: CurrencyFormat,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum DateFormat {
    SystemLocale,
    Iso,
    DayMonthYear,
    MonthDayYear,
    Short,
    Long,
    Raw,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum TimeFormat {
    SystemLocale,
    HoursMinutes,
    HoursMinutesSeconds,
    TwelveHour,
    Fractional,
    Raw,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum CurrencyFormat {
    SystemLocale,
    Raw,
    Usd,
    Eur,
    Gbp,
    Jpy,
    Krw,
    Cny,
    Brl,
    Rub,
    Cad,
    Aud,
    Inr,
    Chf,
    Kwd,
    Bhd,
    Tnd,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Match {
    Any,
    Whole,
    Start,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Direction {
    All,
    Up,
    Down,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ContinueFind {
    document_revision: u64,
    find_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReplaceFound<'a> {
    document_revision: u64,
    find_id: u64,
    replacement: &'a str,
}

pub(super) fn schema(operation: &str) -> Option<Map<String, Value>> {
    let schema = match operation {
        "find_next" => schemars::schema_for!(Find<'_>),
        "continue_find" => schemars::schema_for!(ContinueFind),
        "replace_found" => schemars::schema_for!(ReplaceFound<'_>),
        "list_tables" | "session_status" => schemars::schema_for!(Empty),
        "open_database" => schemars::schema_for!(Open<'_>),
        "create_database" | "start_reserved_recovery" => schemars::schema_for!(PathRequest<'_>),
        "inspect_database_options" | "review_disable_name_auto_correct" => {
            schemars::schema_for!(Revision)
        }
        "describe_table"
        | "begin_insert"
        | "inspect_field_addition"
        | "inspect_table_drop"
        | "review_table_drop" => schemars::schema_for!(Table<'_>),
        "read_rows" => schemars::schema_for!(Page<'_>),
        "inspect_collection" | "begin_edit" => schemars::schema_for!(Cell<'_>),
        "review_delete" => schemars::schema_for!(Row<'_>),
        "inspect_query" => schemars::schema_for!(Query<'_>),
        "run_query" | "review_query_insert" | "review_query_delete" | "review_query_update" => {
            schemars::schema_for!(QueryRun<'_>)
        }
        "inspect_saved_query" | "inspect_query_sql" => {
            schemars::schema_for!(Saved<'_>)
        }
        "run_saved_query" => schemars::schema_for!(SavedRun<'_>),
        "review_edit" => schemars::schema_for!(Edit<'_>),
        "review_insert" => schemars::schema_for!(Insert<'_>),
        "review_table_rename" => schemars::schema_for!(TableRename<'_>),
        "review_table_creation" => schemars::schema_for!(TableCreation<'_>),
        "review_field_addition" => schemars::schema_for!(FieldAddition<'_>),
        "inspect_field_rename" | "inspect_field_drop" | "review_field_drop" => {
            schemars::schema_for!(FieldTarget<'_>)
        }
        "review_field_rename" => schemars::schema_for!(FieldRename<'_>),
        "review_query_sql" => schemars::schema_for!(QuerySql<'_>),
        "review_query_expression" => schemars::schema_for!(Expression<'_>),
        "save_status"
        | "cancel_save"
        | "recover_save"
        | "release_save"
        | "acknowledge_save"
        | "retry_insert"
        | "discard_insert_retry" => {
            schemars::schema_for!(Save)
        }
        "creation_status"
        | "cancel_creation"
        | "acknowledge_creation"
        | "open_created_database" => schemars::schema_for!(Creation),
        "apply_edit"
        | "apply_insert"
        | "apply_delete"
        | "apply_table_rename"
        | "apply_table_creation"
        | "apply_table_drop"
        | "apply_field_addition"
        | "apply_field_rename"
        | "apply_field_drop"
        | "apply_database_options"
        | "apply_query_sql"
        | "apply_query_expression"
        | "apply_query_insert"
        | "apply_query_delete"
        | "apply_query_update" => schemars::schema_for!(Apply),
        _ => return filtering::schema(operation).or_else(|| query_design::schema(operation)),
    };
    match schema.to_value() {
        Value::Object(object) => Some(object),
        _ => None,
    }
}

pub(super) fn validate(operation: &str, arguments: &Value) -> Result<(), String> {
    match operation {
        "find_next" => {
            let request = Find::deserialize(arguments).map_err(|error| error.to_string())?;
            if request.formatted.unwrap_or(false) && request.format_options.is_none() {
                return Err("Formatted Find requires explicit format_options".into());
            }
            Ok(())
        }
        "continue_find" => check::<ContinueFind>(arguments),
        "replace_found" => check::<ReplaceFound<'_>>(arguments),
        "list_tables" | "session_status" => check::<Empty>(arguments),
        "open_database" => check::<Open<'_>>(arguments),
        "create_database" | "start_reserved_recovery" => check::<PathRequest<'_>>(arguments),
        "inspect_database_options" | "review_disable_name_auto_correct" => {
            check::<Revision>(arguments)
        }
        "describe_table"
        | "begin_insert"
        | "inspect_field_addition"
        | "inspect_table_drop"
        | "review_table_drop" => check::<Table<'_>>(arguments),
        "read_rows" => check::<Page<'_>>(arguments),
        "inspect_collection" | "begin_edit" => check::<Cell<'_>>(arguments),
        "review_delete" => check::<Row<'_>>(arguments),
        "inspect_query" => check::<Query<'_>>(arguments),
        "run_query" | "review_query_insert" | "review_query_delete" | "review_query_update" => {
            check::<QueryRun<'_>>(arguments)
        }
        "inspect_saved_query" | "inspect_query_sql" => check::<Saved<'_>>(arguments),
        "run_saved_query" => check::<SavedRun<'_>>(arguments),
        "review_edit" => check::<Edit<'_>>(arguments),
        "review_insert" => check::<Insert<'_>>(arguments),
        "review_table_rename" => check::<TableRename<'_>>(arguments),
        "review_table_creation" => check::<TableCreation<'_>>(arguments),
        "review_field_addition" => check::<FieldAddition<'_>>(arguments),
        "inspect_field_rename" | "inspect_field_drop" | "review_field_drop" => {
            check::<FieldTarget<'_>>(arguments)
        }
        "review_field_rename" => check::<FieldRename<'_>>(arguments),
        "review_query_sql" => check::<QuerySql<'_>>(arguments),
        "review_query_expression" => check::<Expression<'_>>(arguments),
        "save_status"
        | "cancel_save"
        | "recover_save"
        | "release_save"
        | "acknowledge_save"
        | "retry_insert"
        | "discard_insert_retry" => check::<Save>(arguments),
        "creation_status"
        | "cancel_creation"
        | "acknowledge_creation"
        | "open_created_database" => check::<Creation>(arguments),
        "apply_edit"
        | "apply_insert"
        | "apply_delete"
        | "apply_table_rename"
        | "apply_table_creation"
        | "apply_table_drop"
        | "apply_field_addition"
        | "apply_field_rename"
        | "apply_field_drop"
        | "apply_database_options"
        | "apply_query_sql"
        | "apply_query_expression"
        | "apply_query_insert"
        | "apply_query_delete"
        | "apply_query_update" => check::<Apply>(arguments),
        _ => filtering::validate(operation, arguments)
            .or_else(|| query_design::validate(operation, arguments))
            .unwrap_or_else(|| Err("Unknown TableCraft tool".into())),
    }
}

fn check<'a, T: Deserialize<'a>>(arguments: &'a Value) -> Result<(), String> {
    T::deserialize(arguments)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
