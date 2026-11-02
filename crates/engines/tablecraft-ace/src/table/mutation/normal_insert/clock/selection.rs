//! Read-only original-presence default selection; this issues no Source authority.
use crate::table::mutation::insert_defaults::{
    declaration,
    literal::{self, Bound},
};
use crate::{
    EditError, InsertFieldValue, InspectError, NativeImage, OmittedRowInsert, WriteBlocker,
    header::check, inspect_table_properties, inspect_table_schema, limits::MAX_LIVE_ROWS,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Classify needed exact Date()/Now() declarations without reading a clock
    /// or target records. Native Source and all constraints are checked afresh
    /// by the selected planner; this bool cannot authorize a mutation.
    pub fn select_native_ordinary_clock_defaults(
        &self,
        table: &str,
        requests: &[OmittedRowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        if requests.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let mut omitted = false;
        for request in requests {
            check(budget, cancelled)?;
            if request.values().len() != schema.columns().len() {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            for field in request.values() {
                check(budget, cancelled)?;
                omitted |= matches!(field, InsertFieldValue::Omitted);
            }
        }
        if !omitted {
            return Ok(false);
        }
        let properties = inspect_table_properties(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
            return Err(EditError::Preservation);
        }
        let mut needed = false;
        for request in requests {
            check(budget, cancelled)?;
            for (column, field) in schema.columns().iter().zip(request.values()) {
                check(budget, cancelled)?;
                if matches!(field, InsertFieldValue::Omitted)
                    && let Some(property) =
                        declaration::capture(column, &properties, budget, cancelled)?
                {
                    needed |= matches!(
                        literal::bind_clock(column, property.text()?, budget, cancelled)?,
                        Bound::Clock(_)
                    );
                }
            }
        }
        check(budget, cancelled)?;
        Ok(needed)
    }
}
