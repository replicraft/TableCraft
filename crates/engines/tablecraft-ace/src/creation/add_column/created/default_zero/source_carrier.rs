//! Choose one explicit Number source purpose from actual native field framing.
use super::CreatedDefaultZeroAddColumnError;
use crate::{
    CreatedAddColumnError, FileFormat, NativeImage, header::check, inspect_table_schema,
    table::mutation::catalog_property,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Observe whether this table's actual catalog property carrier is external.
    ///
    /// Logical-root resolution includes native forwarding. This inspects no
    /// property program, grants no write admission and never retries a failed
    /// constructor. The selected source must independently qualify completely.
    pub fn number_column_uses_external_properties(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, CreatedDefaultZeroAddColumnError> {
        // Retain the original Number source's capture/format refusal before
        // attempting metadata classification on another native file family.
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        if self.catalog().header().format() != FileFormat::Ace2016 {
            return Err(CreatedAddColumnError::Source.into());
        }
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        Ok(catalog_property::properties_external(
            self, &schema, budget, cancelled,
        )?)
    }
}

#[cfg(test)]
mod tests;
