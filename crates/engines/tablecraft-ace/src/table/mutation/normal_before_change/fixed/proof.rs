//! Fresh original-request replay plus the independent joint raw image proof.
use super::{capture, event_error, preflight, prepare, validate_original};
use crate::{
    EditError, FixedColumnUpdates, NativeEditPlan, NativeImage,
    header::check,
    table::mutation::{
        fixed_records::{self, proof},
        normal_before_change::operands::original_frames,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.before_change.normal.fixed.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = capture(&image, table, budget, cancelled)?;
    original_frames(&source, budget, cancelled)?;
    preflight(&image, &source, original, budget, cancelled)?;
    validate_original(&source, original, budget, cancelled)?;
    let prepared = prepare(&image, &source, original, budget, cancelled)?;
    if prepared.rejection().is_some() {
        return Err(EditError::Preservation);
    }
    prepared.require_source(&image, source.table.schema(), budget, cancelled)?;
    let groups = prepared
        .groups(source.table.schema(), budget, cancelled)
        .map_err(event_error)?;
    let expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    proof::prove_original(
        proof::Original {
            image: &image,
            source: &source,
        },
        expected,
        &groups,
        proof::Observed {
            bytes: plan.after_bytes(),
            changed_rows: plan.changed_rows(),
        },
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(())
}
