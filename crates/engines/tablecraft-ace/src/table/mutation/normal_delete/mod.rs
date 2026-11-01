//! Fixed-length ordinary DELETE with fresh program absence and raw-row proof.
mod indexed;
mod proof;
mod selection;
mod stage;

#[cfg(test)]
mod tests;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowDelete, WriteBlocker,
    header::check,
    inspect_table_schema,
    table::mutation::{fixed_records, normal_ordinary, preserved_source::Source, rows},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const NATIVE_CATALOG_FIELDS: usize = 17;
const CATALOG_NAME: &str = "MSysObjects";

impl NativeImage {
    /// Plan exact original ordinary-row deletions without reclaiming payloads.
    pub(crate) fn plan_ordinary_row_deletes(
        self,
        table: &str,
        deletes: &[RowDelete],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.delete.plan").entered();
        let source = capture(&self, table, budget, cancelled)?;
        rows::unique(deletes, budget, cancelled)?;
        for request in deletes {
            check(budget, cancelled)?;
            if request.expected_bytes().len() > crate::format::page::PAGE_BYTES {
                return Err(InspectError::StructureLimit.into());
            }
        }

        let staged = stage::prepare(&self, &source, deletes, budget, cancelled)?;
        let indexes = if source.indexes.indexes().is_empty() {
            Vec::new()
        } else {
            indexed::patches(&self, &source, deletes, budget, cancelled)?
        };
        let mut after = fixed_records::copy(self.as_bytes(), budget, cancelled)?;
        staged.apply(&mut after, budget, cancelled)?;
        for patch in indexes {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, patch.before.len() as u64)
                .map_err(InspectError::from)?;
            let end = patch
                .offset
                .checked_add(patch.before.len())
                .ok_or(EditError::Preservation)?;
            let output = after
                .get_mut(patch.offset..end)
                .ok_or(EditError::Preservation)?;
            if output != patch.before {
                return Err(EditError::Preservation);
            }
            output.copy_from_slice(&patch.after);
        }

        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: deletes.len(),
        };
        proof::prove(&plan, scope, table, deletes, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}

fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    // Global closed-content DATA framing currently excludes tombstones. Keep
    // that separate profile unchanged: this action consumes the actual native
    // target-local NoRequired certificate, selected from original metadata.
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
    if !source.indexes.indexes().is_empty() {
        indexed::qualify(image, &source, budget, cancelled)?;
    }
    // Source capture already rejects every relationship involving this table.
    // Unrelated declarations remain protected and do not authorize cascades.
    check(budget, cancelled)?;
    Ok(source)
}

#[cfg(test)]
mod node_sources;
