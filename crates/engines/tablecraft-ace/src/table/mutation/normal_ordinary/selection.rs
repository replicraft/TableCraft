//! Metadata selects an attempt; only a fresh complete content proof admits it.
use super::{capture, preflight};
use crate::{
    EditError, FileFormat, FixedColumnUpdates, NativeImage, PayloadPresence, WriteBlocker,
    format::{
        data_type::{BOOLEAN, CURRENCY, DATE_TIME, INT32, INT64},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    inspect_table_schema,
    table::mutation::fixed_records::{binding, indexed::i32_updates::I32UpdateIndexes},
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
        let mut indexed_column = None;
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
            if !supported
                || !field.fixed()
                || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || field.extended_flags() != 0
            {
                continue;
            }
            let mut unavailable = false;
            for dependency in &source.dependencies {
                check(budget, cancelled)?;
                unavailable |= *dependency == field.number();
            }
            if unavailable {
                continue;
            }
            let mut indexed = false;
            for index in source.indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    indexed |= key.number() == field.number();
                }
            }
            if indexed {
                // Complete metadata selection precedes the indexed service:
                // an unchanged wide index must not hide an eligible unindexed
                // sibling just because indexed Id occurs earlier in the schema.
                if indexed_column.is_none() {
                    indexed_column = Some(field.name());
                }
                continue;
            }
            let groups = [FixedColumnUpdates::new(
                source.table.schema().name(),
                field.name(),
                &[],
            )];
            return preflight(&source, &groups, budget, cancelled);
        }
        let column = indexed_column.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        let groups = [FixedColumnUpdates::new(
            source.table.schema().name(),
            column,
            &[],
        )];
        if super::wide::selected(&source, budget, cancelled)? {
            let facts = super::wide::capture_indexes(self, &source, budget, cancelled)?;
            binding::all_integral(self, &source, &groups, &facts, budget, cancelled)?;
            return Ok(());
        }
        let facts = I32UpdateIndexes::capture(self, table, &source, budget, cancelled)?;
        binding::all_i32_updates(self, &source, &groups, &facts, budget, cancelled)?;
        Ok(())
    }
}
