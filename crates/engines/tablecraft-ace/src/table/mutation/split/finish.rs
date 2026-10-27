//! Append-aware split certification returns no journalable edit plan.
use super::super::{Ace16WritePreview, EditError, WriteBlocker, gates, index_append};
use super::{Operation, Staged, binding::Binding};
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints, header::check,
    inspect_catalog,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) fn finish(
    image: NativeImage,
    operation: &Operation<'_>,
    staged: &Staged,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    bound(image, operation, staged, budget, cancelled)
}

pub(super) fn bound<const WIDTH: usize>(
    image: NativeImage,
    operation: &impl Binding<WIDTH>,
    staged: &Staged<WIDTH>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let table = operation.table();
    let structural = &staged.structural;
    let fields = &staged.patches;
    let plan = &staged.plan;
    check(budget, cancelled)?;
    index_append::qualify(&image, plan)?;
    let keys = super::authority::prove(&image, operation, structural, fields, budget, cancelled)?;
    let after = index_append::compose(&image, fields, plan, budget, cancelled)?;
    if plan.promotions.is_empty() {
        plan.audit(&image, structural, &keys, &after, budget, cancelled)?;
    } else {
        // Map metadata rewrite authority binds the immutable source operation,
        // independently of the schema/index metadata carried by staging.
        let original = check_table(
            &mut Cursor::new(image.as_bytes()),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        plan.audit(&image, &original, &keys, &after, budget, cancelled)?;
    }
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
    check(budget, cancelled)?;
    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        fields.len(),
    ))
}
