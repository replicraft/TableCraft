//! Native17 normal updates may allocate exactly one represented DATA page at EOF.
mod proof;
mod selection;
mod stage;

pub(crate) use selection::OrdinaryVariablePlanKind;

use crate::{
    EditError, FittedColumnUpdates, NativeEditPlan, NativeImage, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_schema,
    table::mutation::{
        normal_ordinary,
        preserved_source::{Source, validation},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

const NATIVE_CATALOG_FIELDS: usize = 17;
const CATALOG_NAME: &str = "MSysObjects";

impl NativeImage {
    pub(crate) fn plan_ordinary_eof_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.eof.plan").entered();
        check(budget, cancelled)?;
        let source = capture(&self, table, budget, cancelled)?;
        let (after, changed_rows) = stage::build(&self, &source, groups, budget, cancelled)?;
        if self.as_bytes().len().checked_add(PAGE_BYTES) != Some(after.len()) {
            return Err(EditError::Preservation);
        }
        validate_after(&self, table, &source, &after, budget, cancelled)?;
        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows,
        };
        proof::prove(&plan, scope, table, groups, budget, cancelled)?;
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
    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        CATALOG_NAME,
        budget,
        cancelled,
    )?;
    if catalog.columns().len() != NATIVE_CATALOG_FIELDS {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    normal_ordinary::capture(image, table, budget, cancelled)
}

fn validate_after(
    image: &NativeImage,
    table: &str,
    source: &Source,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    validation::expected(image, table, &source.table, after, budget, cancelled)?;
    let after_image = NativeImage::capture(
        &mut Cursor::new(after),
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    let checked = capture(&after_image, table, budget, cancelled)?;
    if checked.table.schema().object_id() != source.table.schema().object_id()
        || checked.table.rows().len() != source.table.rows().len()
    {
        return Err(EditError::Preservation);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
