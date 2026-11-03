//! Metadata selects an attempt; only a fresh complete content proof admits it.
use super::{capture, preflight, preflight_with_policy};
use crate::{
    EditError, FileFormat, FixedColumnUpdates, NativeImage, PayloadPresence, WriteBlocker,
    format::{
        data_type::{
            BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64, SINGLE,
        },
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    inspect_table_schema,
    table::mutation::fixed_records::{
        FixedCarriers, binding, indexed::i32_updates::I32UpdateIndexes,
    },
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

    /// Select only a request dialect from original metadata; native admission
    /// and every target check are still repeated by the consuming planner.
    pub(crate) fn ordinary_fixed_carriers(
        &self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FixedCarriers, EditError> {
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let mut carriers = FixedCarriers::Legacy;
        for group in groups {
            check(budget, cancelled)?;
            for field in schema.columns() {
                check(budget, cancelled)?;
                super::fixed_records::name_work(field.name(), group.column(), budget)?;
                if field.name().eq_ignore_ascii_case(group.column()) {
                    carriers = match field.native_type() {
                        GUID => FixedCarriers::Guid,
                        BYTE | INT16 if carriers != FixedCarriers::Guid => FixedCarriers::Scalar,
                        SINGLE | DOUBLE if carriers == FixedCarriers::Legacy => FixedCarriers::Ieee,
                        _ => carriers,
                    };
                }
            }
        }
        check(budget, cancelled)?;
        Ok(carriers)
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
                    | (SINGLE, Some(ScalarType::Single))
                    | (DOUBLE, Some(ScalarType::Double))
                    | (BYTE, Some(ScalarType::Byte))
                    | (INT16, Some(ScalarType::Int16))
                    | (GUID, Some(ScalarType::Guid))
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
            let carriers = match field.native_type() {
                GUID => FixedCarriers::Guid,
                BYTE | INT16 => FixedCarriers::Scalar,
                SINGLE | DOUBLE => FixedCarriers::Ieee,
                _ => FixedCarriers::Legacy,
            };
            if carriers != FixedCarriers::Legacy {
                return preflight_with_policy(&source, &groups, carriers, budget, cancelled);
            }
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
