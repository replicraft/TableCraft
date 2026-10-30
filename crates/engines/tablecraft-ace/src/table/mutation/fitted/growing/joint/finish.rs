//! Independently establish both phases before returning any candidate.
use super::super::super::request::FittedColumnUpdates;
use super::{Staged, certified};
use crate::{
    Ace16WritePreview, EditError, IndexCheckStatus, NativeImage, WriteBlocker, check_table,
    check_table_constraints,
    header::check,
    inspect_catalog,
    table::mutation::{gates, index_append},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) fn finish(
    image: NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    staged: &Staged,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    check(budget, cancelled)?;
    let data = staged.data.view(&staged.indexes.append);
    let after = index_append::compose_data(&image, data, &staged.indexes, budget, cancelled)?;
    let certified = certified::certify(&image, table, groups, data, &after, budget, cancelled)?;
    let changed = certified.changed_rows();
    if changed != staged.changed {
        return Err(EditError::Preservation);
    }

    staged
        .indexes
        .audit_rows_after_data(&image, &certified, &after, budget, cancelled)?;

    let scope = image.catalog().catalog().scope();
    inspect_catalog(&mut Cursor::new(&after), scope, budget, cancelled)?;
    let checked = check_table(&mut Cursor::new(&after), scope, table, budget, cancelled)?;
    if checked
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    gates::qualified(&check_table_constraints(
        &mut Cursor::new(&after),
        scope,
        table,
        budget,
        cancelled,
    )?)?;
    staged
        .data
        .audit_membership(&after, checked.schema(), budget, cancelled)?;
    check(budget, cancelled)?;

    drop(certified);
    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        changed,
    ))
}
