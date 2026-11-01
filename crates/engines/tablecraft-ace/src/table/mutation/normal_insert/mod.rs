//! Normal complete-row INSERT into protected, already-owned DATA capacity.
mod indexed;
mod omitted;
mod placement;
mod proof;
mod records;

#[cfg(test)]
mod tests;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_schema,
    limits::MAX_LIVE_ROWS,
    table::mutation::{
        fixed_records, normal_ordinary, preserved_source::Source, record_insertion::properties,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG_NAME: &str = "MSysObjects";
const NATIVE_CATALOG_FIELDS: usize = 17;

impl NativeImage {
    /// Plan complete explicitly supplied rows without defaults or allocation.
    pub(crate) fn plan_ordinary_row_inserts(
        self,
        table: &str,
        requests: &[RowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.insert.plan").entered();
        let source = capture_source(&self, table, budget, cancelled)?;
        if !source.indexes.indexes().is_empty() {
            return indexed::plan(self, source, table, requests, budget, cancelled);
        }

        source
            .records
            .len()
            .checked_add(requests.len())
            .filter(|count| *count <= MAX_LIVE_ROWS)
            .ok_or(InspectError::StructureLimit)?;

        properties::require_text_declarations(
            &self,
            source.table.schema(),
            requests,
            budget,
            cancelled,
        )?;
        records::qualify(source.table.schema(), requests, budget, cancelled)?;

        let encoded = records::encode(source.table.schema(), requests, budget, cancelled)?;

        let staged = placement::reserve(&self, &source, &encoded, budget, cancelled)?;

        let mut after = fixed_records::copy(self.as_bytes(), budget, cancelled)?;
        staged.apply(&self, &encoded, &mut after, budget, cancelled)?;

        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: requests.len(),
        };

        proof::prove(&plan, scope, table, requests, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}

fn capture_source(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        CATALOG_NAME,
        budget,
        cancelled,
    )?;
    if catalog.columns().len() != NATIVE_CATALOG_FIELDS {
        return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
    }
    let source = normal_ordinary::capture(image, table, budget, cancelled)?;

    // Source already refuses any relationship involving this target and owns
    // complete properties, dependencies, catalog and LVAL preservation.
    if source.records.len() != source.table.rows().len()
        || usize::try_from(source.table.schema().declared_rows()).ok() != Some(source.records.len())
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(source)
}

fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let source = capture_source(image, table, budget, cancelled)?;
    if !source.indexes.indexes().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    Ok(source)
}

fn compare(
    expected: &[u8],
    observed: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if expected.len() != observed.len() {
        return Err(EditError::Preservation);
    }
    for (expected, observed) in expected.chunks(PAGE_BYTES).zip(observed.chunks(PAGE_BYTES)) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != observed {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod review_tests;

#[cfg(test)]
mod feff_tests;

#[cfg(test)]
mod node_sources;

#[cfg(test)]
mod inline_tests;
