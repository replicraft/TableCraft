//! Metadata-only complete-row INSERT preparation; no concrete storage or SQL.
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::{Column, SchemaPolicy};

/// One immutable insertion target and one consuming, nonpersistent row finish.
/// The supplied columns describe every requested value in final backend order.
/// A backend may expose an explicitly selected companion schema, but must keep
/// generation/default behavior at its own boundary. Schema inspection must not
/// scan records or perform durable writes.
pub trait InsertableTable: Sized {
    /// Sanitized backend refusal with its typed cause retained.
    type Error: std::error::Error;
    /// Complete backend candidate, never an implicit commit or publication.
    type Plan;

    /// Canonical local target name.
    fn name(&self) -> &str;
    /// Complete supplied-column schema, with no implicit omitted fields.
    fn columns(&self) -> &[Column];
    /// Explicit name comparison and metadata limits for this target.
    fn schema_policy(&self) -> SchemaPolicy;
    /// Whether explicit field names are required for this selected supplied
    /// schema. Companion schemas require this so positional VALUES cannot be
    /// silently reinterpreted as omitting a native generated field.
    fn requires_field_list(&self) -> bool;
    /// Consume all values once, rejecting wrong width/carriers and unavailable
    /// backend constraints. Failure returns no partial candidate. No durable
    /// writes or default evaluation are implied by this neutral capability.
    fn plan_insert(
        self,
        values: Vec<Value>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error>;
}
