//! Read-only wire overrides bind the runtime's validated per-run allowances.
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use tablecraft_runtime::{QueryPoolLimits, QueryRunOptions, QueryRunOptionsError};

/// Omitted or null fields retain the shared runtime defaults for this run.
#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadQueryOptions {
    /// Positive complete-table row allowance within the u32 request representation.
    snapshot_rows: Option<u32>,
    /// Positive native fetch size within the u32 request representation; independent of snapshot rows.
    batch_rows: Option<u32>,
    native: Option<PoolOverride>,
    execution: Option<PoolOverride>,
    comparison: Option<PoolOverride>,
    presentation: Option<PoolOverride>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PoolOverride {
    /// Positive cumulative allocation allowance within the u64 counter representation.
    bytes: Option<u64>,
    /// Cumulative row allowance within the u64 counter representation.
    /// Native/execution require positive values; comparison/presentation require zero.
    rows: Option<u64>,
    /// Positive cumulative work allowance within the u64 counter representation.
    steps: Option<u64>,
}

impl ReadQueryOptions {
    fn resolve(self) -> Result<QueryRunOptions, QueryRunOptionsError> {
        let defaults = QueryRunOptions::default();
        let snapshot = defaults.snapshot();
        QueryRunOptions::with_limits(
            self.snapshot_rows.unwrap_or(snapshot.maximum_rows()),
            self.batch_rows.unwrap_or(snapshot.batch_rows()),
            pool(self.native, defaults.native())?,
            pool(self.execution, defaults.query())?,
            pool(self.comparison, defaults.comparison())?,
            pool(self.presentation, defaults.presentation())?,
        )
    }
}

fn pool(
    request: Option<PoolOverride>,
    defaults: QueryPoolLimits,
) -> Result<QueryPoolLimits, QueryRunOptionsError> {
    let Some(request) = request else {
        return Ok(defaults);
    };
    QueryPoolLimits::new(
        request.bytes.unwrap_or(defaults.bytes()),
        request.rows.unwrap_or(defaults.rows()),
        request.steps.unwrap_or(defaults.steps()),
    )
}

pub(crate) fn options(arguments: &Value) -> Result<QueryRunOptions, String> {
    let request = match arguments.get("options").filter(|value| !value.is_null()) {
        Some(value) => ReadQueryOptions::deserialize(value).map_err(|error| error.to_string())?,
        None => ReadQueryOptions::default(),
    };
    request.resolve().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
