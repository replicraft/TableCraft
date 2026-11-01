//! Metadata selects an attempt; only a fresh complete content proof admits it.
use super::{capture, preflight};
use crate::{
    EditError, FileFormat, FixedColumnUpdates, NativeImage, PayloadPresence, WriteBlocker,
    format::data_type::{BOOLEAN, CURRENCY, DATE_TIME, INT32, INT64},
    header::check,
    inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

impl NativeImage {
    /// Choose an ordinary-content capability attempt from inert catalog facts.
    /// This does not prove absence, grant a write profile or retain permission.
    /// Positive and uninspected carriers stay with their existing service.
    pub fn select_native_ordinary_fixed_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let _span = tracing::debug_span!("native.ordinary.fixed.select").entered();
        check(budget, cancelled)?;
        if !matches!(
            self.catalog().header().format(),
            FileFormat::Ace2016 | FileFormat::Ace2019
        ) {
            return Ok(false);
        }
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let payloads = self.inspect_object_payloads(schema.object_id(), budget, cancelled)?;
        if payloads.body() != PayloadPresence::Absent
            || payloads.module() != PayloadPresence::Absent
            || payloads.extra() != PayloadPresence::Absent
        {
            return Ok(false);
        }
        let children = self.inspect_object_children(schema.object_id(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(children.is_empty())
    }

    /// Prove complete original content and qualify an actual supported fixed
    /// target. This evaluates no program, stages no assignment and issues no
    /// reusable permission; every subsequent plan repeats the complete proof.
    pub fn check_native_ordinary_fixed_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let _span = tracing::debug_span!("native.ordinary.fixed.table").entered();
        let source = capture(self, table, budget, cancelled)?;
        for field in source.table.schema().columns() {
            check(budget, cancelled)?;
            let supported = matches!(
                (field.native_type(), field.scalar_type()),
                (BOOLEAN, Some(ScalarType::Boolean))
                    | (INT32, Some(ScalarType::Int32))
                    | (INT64, Some(ScalarType::Int64))
                    | (CURRENCY, Some(ScalarType::Currency))
                    | (DATE_TIME, Some(ScalarType::DateTime))
            );
            if !supported {
                continue;
            }
            let groups = [FixedColumnUpdates::new(
                source.table.schema().name(),
                field.name(),
                &[],
            )];
            return preflight(&source, &groups, budget, cancelled);
        }
        Err(EditError::Unsupported(WriteBlocker::Column))
    }
}
