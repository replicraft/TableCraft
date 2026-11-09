//! The same checked headless draft builder supplies native table review snapshots.
use super::arg;
use serde_json::Value;
use tablecraft_builders::table::{
    DraftDefaultMode, DraftFieldType, DraftNumberSize, TableDraft, TableDraftSnapshot,
};
use tablecraft_schema::DecimalConstraints;

pub(super) fn snapshot(args: &Value) -> Result<TableDraftSnapshot, String> {
    let mut draft = if arg::boolean(args, "initial_autonumber", true)? {
        TableDraft::initial()
    } else {
        TableDraft::design()
    };
    let fields = args
        .get("fields")
        .and_then(Value::as_array)
        .ok_or("fields must be an array")?;
    for field in fields {
        let kind = match arg::text(field, "kind")? {
            "short_text" => DraftFieldType::ShortText,
            "number" => DraftFieldType::Number,
            "large_integer" => DraftFieldType::LargeInteger,
            "yes_no" => DraftFieldType::YesNo,
            "currency" => DraftFieldType::Currency,
            "date_time" => DraftFieldType::DateTime,
            _ => return Err("Unsupported field kind".into()),
        };
        let id = draft
            .add_named_field(kind, arg::text(field, "name")?)
            .map_err(|error| error.to_string())?;
        if let Some(units) = arg::optional(field, "max_utf16_units") {
            draft
                .set_field_text_units(
                    id,
                    u16::try_from(units.as_u64().ok_or("max_utf16_units must be an integer")?)
                        .map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
        }
        if arg::optional(field, "required").is_some() {
            draft
                .set_field_required(id, Some(arg::boolean(field, "required", false)?))
                .map_err(|error| error.to_string())?;
        }
        if arg::optional(field, "allow_zero_length").is_some() {
            draft
                .set_field_allow_zero_length(
                    id,
                    Some(arg::boolean(field, "allow_zero_length", false)?),
                )
                .map_err(|error| error.to_string())?;
        }
        let literal = arg::optional(field, "default_literal");
        let mode = arg::optional(field, "default_mode");
        if literal.is_some() || mode.is_some() {
            let requested = match mode {
                None => "literal",
                Some(_) => arg::text(field, "default_mode")?,
            };
            if requested != "literal" && literal.is_some() {
                return Err("default_literal requires literal default_mode".into());
            }
            let mode = match requested {
                "type_default" => DraftDefaultMode::TypeDefault,
                "absent" => DraftDefaultMode::Absent,
                "null" => DraftDefaultMode::Null,
                "literal" => DraftDefaultMode::Literal(arg::owned(field, "default_literal")?),
                _ => return Err("Unsupported default_mode".into()),
            };
            draft
                .set_field_default(id, mode)
                .map_err(|error| error.to_string())?;
        }
        if arg::optional(field, "description").is_some() {
            draft
                .set_field_description(id, Some(arg::text(field, "description")?))
                .map_err(|error| error.to_string())?;
        }
        if let Some(size) = arg::optional(field, "number_size") {
            let size = match size.as_str().ok_or("number_size must be a string")? {
                "byte" => DraftNumberSize::Byte,
                "integer" => DraftNumberSize::Integer,
                "long_integer" => DraftNumberSize::LongInteger,
                "single" => DraftNumberSize::Single,
                "double" => DraftNumberSize::Double,
                "decimal" => DraftNumberSize::Decimal,
                "replication_id" => DraftNumberSize::ReplicationId,
                _ => return Err("Unsupported number_size".into()),
            };
            draft
                .set_field_number_size(id, size)
                .map_err(|error| error.to_string())?;
        }
        if arg::optional(field, "precision").is_some() != arg::optional(field, "scale").is_some() {
            return Err("Decimal precision and scale must be supplied together".into());
        }
        if arg::optional(field, "precision").is_some() {
            let precision = u8::try_from(arg::u64_value(field, "precision")?)
                .map_err(|error| error.to_string())?;
            let scale =
                u8::try_from(arg::u64_value(field, "scale")?).map_err(|error| error.to_string())?;
            draft
                .set_field_decimal_constraints(
                    id,
                    DecimalConstraints::new(precision, scale).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
        }
        if arg::boolean(field, "primary", false)? {
            draft
                .set_primary_key(Some(id))
                .map_err(|error| error.to_string())?;
        }
    }
    draft
        .snapshot(arg::text(args, "table")?)
        .map_err(|error| error.to_string())
}
