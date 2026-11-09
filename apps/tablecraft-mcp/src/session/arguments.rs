//! Literal protocol input; runtime remains the owner of semantic conversion.
use serde_json::Value;
use tablecraft_runtime::{EditInput, ParameterInput, SavedQueryTarget};

pub(super) fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Expected string argument {key}"))
}

pub(super) fn owned(args: &Value, key: &str) -> Result<String, String> {
    // NO_CLONE: Runtime command carriers cross their worker thread boundary and require
    // independent owned text; the protocol input cannot be borrowed beyond this call.
    Ok(text(args, key)?.to_owned())
}

pub(super) fn u64_value(args: &Value, key: &str) -> Result<u64, String> {
    args.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("Expected unsigned integer argument {key}"))
}

pub(super) fn usize_value(args: &Value, key: &str) -> Result<usize, String> {
    usize::try_from(u64_value(args, key)?).map_err(|error| error.to_string())
}

pub(super) fn u32_value(args: &Value, key: &str) -> Result<u32, String> {
    u32::try_from(u64_value(args, key)?).map_err(|error| error.to_string())
}

pub(super) fn optional<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|value| !value.is_null())
}

pub(super) fn boolean(args: &Value, key: &str, default: bool) -> Result<bool, String> {
    match optional(args, key) {
        None => Ok(default),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| format!("Expected Boolean argument {key}")),
    }
}

pub(super) fn input(args: &Value) -> Result<EditInput, String> {
    Ok(EditInput {
        text: owned(args, "text")?,
        is_null: boolean(args, "is_null", false)?,
    })
}

pub(super) fn parameters(args: &Value) -> Result<Vec<ParameterInput>, String> {
    let Some(values) = optional(args, "parameters") else {
        return Ok(Vec::new());
    };
    values
        .as_array()
        .ok_or("parameters must be an array")?
        .iter()
        .map(|value| {
            Ok(ParameterInput {
                name: owned(value, "name")?,
                kind: owned(value, "kind")?,
                text: owned(value, "text")?,
                is_null: boolean(value, "is_null", false)?,
            })
        })
        .collect()
}

pub(super) fn saved(args: &Value, document_revision: u64) -> Result<SavedQueryTarget, String> {
    let object_id = text(args, "object_id")?
        .parse::<u128>()
        .map_err(|error| error.to_string())?;
    Ok(SavedQueryTarget {
        document_revision,
        object_id,
        name: owned(args, "name")?,
    })
}
