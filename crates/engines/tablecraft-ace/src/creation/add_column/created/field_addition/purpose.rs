//! Inert source metadata chooses one explicit qualifier before write admission.
use super::{FieldAdditionError, FieldAdditionTarget};
use crate::{
    FileFormat, InspectError, NativeImage,
    format::{data_type::SHORT_TEXT, layout::UNICODE_COMPRESSION_FLAG},
    header::check,
    inspect_table_properties, inspect_table_schema,
    table::mutation::catalog_property,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// Explicit source purpose; observing it grants no candidate or Save permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatedFieldAdditionSourcePurpose {
    /// Original canonical Number ADD with its frozen TCAC0003 source checks.
    FirstNumber,
    /// Original Number history with its frozen TCAC0004 source checks.
    RepeatedNumber,
    /// Separate typed Number/Short Text history and its complete source proof.
    TypedField,
}

impl NativeImage {
    /// Choose one source purpose from native metadata, never failed admission.
    ///
    /// Short Text requests use the separate typed candidate. Number requests
    /// retain the original first/repeated routes unless actual Short Text
    /// compression or inert policies beyond the old Number source require typed
    /// proof. This uses the existing metadata-only policy observer; no supplied
    /// default is executed, omitted policy inferred or native admission granted.
    pub fn created_field_addition_source_purpose(
        &self,
        table: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CreatedFieldAdditionSourcePurpose, FieldAdditionError> {
        let _span = tracing::debug_span!("native.field_addition.select_source_purpose").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        if self.catalog().header().format() != FileFormat::Ace2016 {
            return Err(FieldAdditionError::Source);
        }
        if matches!(target, FieldAdditionTarget::ShortText { .. }) {
            return Ok(CreatedFieldAdditionSourcePurpose::TypedField);
        }
        let scope = self.catalog().catalog().scope();
        let schema = inspect_table_schema(&mut self.reader(), scope, table, budget, cancelled)?;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.native_type() == SHORT_TEXT
                && field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
            {
                return Ok(CreatedFieldAdditionSourcePurpose::TypedField);
            }
        }
        let properties =
            inspect_table_properties(&mut self.reader(), scope, table, budget, cancelled)?;
        if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
            return Err(InspectError::Malformed.into());
        }
        let policies = super::super::properties::capture(&schema, &properties, budget, cancelled)?;
        for policy in policies {
            check(budget, cancelled)?;
            if policy.required == Some(true)
                || policy.allow_empty.is_some()
                || policy.description.is_some()
                || policy
                    .default
                    .as_ref()
                    .is_some_and(|value| *value != Value::Int32(0))
            {
                return Ok(CreatedFieldAdditionSourcePurpose::TypedField);
            }
        }
        if catalog_property::properties_external(self, &schema, budget, cancelled)? {
            Ok(CreatedFieldAdditionSourcePurpose::RepeatedNumber)
        } else {
            Ok(CreatedFieldAdditionSourcePurpose::FirstNumber)
        }
    }
}

#[cfg(test)]
mod tests;
