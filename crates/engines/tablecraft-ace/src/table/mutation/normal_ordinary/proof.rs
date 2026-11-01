//! Fresh content closure and independent manual fixed-record reconstruction.
use super::{capture, preflight};
use crate::{
    EditError, FixedColumnUpdates, NativeEditPlan, NativeImage,
    header::check,
    table::mutation::fixed_records::{self, proof},
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
    let _span = tracing::debug_span!("native.ordinary.fixed.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = capture(&image, table, budget, cancelled)?;
    preflight(&source, original, budget, cancelled)?;
    let expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;

    // This reconstructs payloads and masks from original records and explicit
    // carriers. It consumes neither staged patches nor the producer's count.
    // Its final validation checks original properties and every AFTER byte.
    proof::prove_original(
        proof::Original {
            image: &image,
            source: &source,
        },
        expected,
        original,
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
