//! Fresh content closure and independent manual fixed-record reconstruction.
use super::{capture, preflight_with_policy};
use crate::{
    EditError, FixedColumnUpdates, NativeEditPlan, NativeImage,
    header::check,
    table::mutation::fixed_records::{self, FixedCarriers, proof},
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
    prove_with_policy(
        plan,
        scope,
        table,
        original,
        FixedCarriers::Legacy,
        budget,
        cancelled,
    )
}

pub(super) fn prove_ieee(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_policy(
        plan,
        scope,
        table,
        original,
        FixedCarriers::Ieee,
        budget,
        cancelled,
    )
}

pub(super) fn prove_scalar(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_policy(
        plan,
        scope,
        table,
        original,
        FixedCarriers::Scalar,
        budget,
        cancelled,
    )
}

pub(super) fn prove_guid(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_policy(
        plan,
        scope,
        table,
        original,
        FixedCarriers::Guid,
        budget,
        cancelled,
    )
}

fn prove_with_policy(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    original: &[FixedColumnUpdates<'_>],
    carriers: FixedCarriers,
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
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    preflight_with_policy(&source, original, carriers, budget, cancelled)?;
    let expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;

    // This reconstructs payloads and masks from original records and explicit
    // carriers. It consumes neither staged patches nor the producer's count.
    // Its final validation checks original properties and every AFTER byte.
    let manual_proof = match carriers {
        FixedCarriers::Legacy => proof::prove_original,
        FixedCarriers::Ieee => proof::prove_ieee_original,
        FixedCarriers::Scalar => proof::prove_scalar_original,
        FixedCarriers::Guid => proof::prove_guid_original,
    };
    manual_proof(
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
    // The manual replay above already compared every byte. Fresh full Source
    // additionally binds the final records and untouched retained index owners.
    let after_image = NativeImage::capture(
        &mut Cursor::new(plan.after_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let final_source = capture(&after_image, table, budget, cancelled)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.table.columns().len() != source.table.columns().len()
        || final_source.table.rows().len() != source.table.rows().len()
        || final_source.records.len() != source.records.len()
        || final_source.indexes.indexes().len() != source.indexes.indexes().len()
    {
        return Err(EditError::Preservation);
    }
    for (before, after) in source.records.iter().zip(&final_source.records) {
        check(budget, cancelled)?;
        if before.reference != after.reference
            || before.offset != after.offset
            || before.data.len() != after.data.len()
        {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod ieee;

#[cfg(test)]
mod small_integral;

#[cfg(test)]
mod guid;
