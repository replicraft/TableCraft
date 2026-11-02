//! Fresh original presence/declarations plus independently projected literals.
use crate::table::mutation::insert_defaults;
use crate::{EditError, NativeEditPlan, NativeImage, OmittedRowInsert, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary_insert.omitted.prove").entered();
    if plan.before_bytes().len() != plan.after_bytes().len()
        || plan.changed_rows() != requests.len()
    {
        return Err(EditError::Preservation);
    }
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::super::capture_source(&image, table, budget, cancelled)?;
    if super::super::incrementing::mode::selected(source.table.schema(), budget, cancelled)? {
        return super::super::incrementing::proof::prove(
            plan, scope, table, requests, budget, cancelled,
        );
    }
    super::super::records::qualify(source.table.schema(), &[], budget, cancelled)?;
    let resolved =
        insert_defaults::derive(&image, source.table.schema(), requests, budget, cancelled)?;
    // Each proof below reconstructs DATA records, independent placement and
    // any INDEX changes from BEFORE and freshly derived original intent.
    // No staged rows, patches, prepared keys or expanded producer cache enter.
    if source.indexes.indexes().is_empty() {
        super::super::proof::prove(plan, scope, table, &resolved, budget, cancelled)?;
    } else {
        super::super::indexed::proof::prove(plan, scope, table, &resolved, budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(())
}
