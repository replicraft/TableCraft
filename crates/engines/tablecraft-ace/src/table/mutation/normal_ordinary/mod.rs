//! Ordinary fixed requests require fresh native content and ownership proof.
mod indexed;
mod proof;
mod selection;
mod wide;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selection_tests;

#[cfg(test)]
mod retained_numeric_sources;

#[cfg(test)]
mod retained_selection_tests;

use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    header::check,
    table::mutation::{
        fixed_records::{self, binding, stage},
        no_required_programs::NoRequiredTablePrograms,
        ordinary_content,
        patch::BytePatch,
        preserved_source::{self, Source, validation},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const NATIVE_CATALOG_FIELDS: usize = 17;
const CATALOG_NAME: &str = "MSysObjects";

const CARRIER_COMPARISON_BYTES: usize = 2 * size_of::<i64>();

impl NativeImage {
    /// Plan original fixed assignments only after proving complete ordinary
    /// content. The caller supplies no absence flag, program or creator token.
    pub(crate) fn plan_ordinary_fixed_updates(
        self,
        table: &str,
        original: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.fixed.plan").entered();
        let source = capture(&self, table, budget, cancelled)?;
        plan(self, source, table, original, false, budget, cancelled)
    }

    /// Ordinary IEEE requests retain separate target and original-intent policy.
    pub(crate) fn plan_ordinary_ieee_updates(
        self,
        table: &str,
        original: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.ieee_fixed.plan").entered();
        let source = capture(&self, table, budget, cancelled)?;
        plan(self, source, table, original, true, budget, cancelled)
    }
}

fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    original: &[FixedColumnUpdates<'_>],
    ieee: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    if indexed::selected(&source, original, budget, cancelled)? {
        if ieee {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if wide::selected(&source, budget, cancelled)? {
            return wide::plan(image, source, table, original, budget, cancelled);
        }
        return indexed::plan(image, source, table, original, budget, cancelled);
    }
    preflight_with_policy(&source, original, ieee, budget, cancelled)?;

    let (patches, changed_rows) = if ieee {
        stage::prepare_ieee(&image, &source, original, budget, cancelled)?
    } else {
        stage::prepare_normal(&image, &source, original, budget, cancelled)?
    };
    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    for patch in patches {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, patch.before().len() as u64)
            .map_err(InspectError::from)?;
        let end = patch
            .offset()
            .checked_add(patch.before().len())
            .ok_or(EditError::Preservation)?;
        let output = after
            .get_mut(patch.offset()..end)
            .ok_or(EditError::Preservation)?;
        if output != patch.before() {
            return Err(EditError::Preservation);
        }
        output.copy_from_slice(patch.after());
    }
    validation::expected(&image, table, &source.table, &after, budget, cancelled)?;

    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows,
    };
    if ieee {
        proof::prove_ieee(&plan, scope, table, original, budget, cancelled)?;
    } else {
        proof::prove(&plan, scope, table, original, budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(plan)
}

pub(in crate::table::mutation) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    // Choose the catalog grammar from original metadata before attempting any
    // content admission. A failed native-profile check never retries globally.
    let catalog = crate::inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        CATALOG_NAME,
        budget,
        cancelled,
    )?;
    if catalog.columns().len() == NATIVE_CATALOG_FIELDS {
        let target = crate::inspect_table_schema(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let certificate =
            NoRequiredTablePrograms::capture_updates(image, &target, budget, cancelled)?;
        return preserved_source::capture_no_required_updates(
            image,
            table,
            &certificate,
            budget,
            cancelled,
        );
    }
    let certificate = ordinary_content::capture(image, budget, cancelled)?;
    preserved_source::capture_closed_content(image, table, &certificate, budget, cancelled)
}

/// Qualify all declarations and carriers before comparing any expected value.
/// Equal supplied values remain ordinary no-ops because the content proof has
/// already established that no supported or unresolved program storage exists.
fn preflight(
    source: &Source,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    preflight_with_policy(source, original, false, budget, cancelled)
}

fn preflight_with_policy(
    source: &Source,
    original: &[FixedColumnUpdates<'_>],
    ieee: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let (bound, _) = if ieee {
        binding::all_ieee(source, original, budget, cancelled)?
    } else {
        binding::all_normal(source, original, budget, cancelled)?
    };
    for (group, selected) in original.iter().zip(bound) {
        check(budget, cancelled)?;
        for update in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, CARRIER_COMPARISON_BYTES as u64)
                .map_err(InspectError::from)?;
            let before = source
                .table
                .rows()
                .get(update.row())
                .and_then(|row| row.get(selected.column))
                .ok_or(EditError::Conflict)?;
            if before != update.expected() {
                return Err(EditError::Conflict);
            }
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod counter_index_tests;

#[cfg(test)]
mod wide_update_original_sources;

#[cfg(test)]
mod wide_update_sources;

#[cfg(test)]
mod wide_node_original_sources;

#[cfg(test)]
mod wide_node_update_sources;

#[cfg(test)]
mod wide_split_original_sources;

#[cfg(test)]
mod wide_root_promotion_sources;

#[cfg(test)]
mod wide_primary_prefix_sources;

#[cfg(test)]
mod wide_leaf_compression_sources;

#[cfg(test)]
mod retained_binary_marker_tests;

#[cfg(test)]
mod generated_companion_original_sources;

#[cfg(test)]
mod generated_companion_update_sources;

#[cfg(test)]
mod generated_companion_update_tests;
