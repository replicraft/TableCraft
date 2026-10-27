//! Separate fixed integral/Currency candidates with qualified recursive index splitting.
mod authority;
mod binding;
mod finish;
mod fixed;
#[cfg(test)]
mod tests;

use super::physical::Patch;
use super::{Ace16WritePreview, EditError, Int32Update, gates, preview};
use crate::{
    InspectError, NativeImage, TableCheck,
    format::reader::Reader,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::indexes::{IndexKeyUpdate, MutationKey, SplitIndexPlan, index_split_patches},
};
use binding::{Binding, Scope};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Preview fixed non-NULL Int32 replacements, including bounded recursive index splits.
    ///
    /// A qualified single-Int32 original root leaf may grow into a height-one
    /// root with two or more represented EOF index leaves. An existing non-root leaf may
    /// add one left sibling while retaining its right half. Overflowing internal
    /// nodes propagate complete child maxima upward; an overflowing root retains
    /// its ID and grows one height. Oversized halves are split again in paid
    /// deterministic passes within the existing codec, depth and map limits.
    /// Every original gate, final constraint and physical row reference is retained.
    /// Unaffected index bytes remain unchanged; no required cascade executes.
    ///
    /// This feature-gated engineering candidate is non-journalable and does not
    /// widen ordinary saves or the existing fixed, page and variable preview APIs.
    /// Microsoft Access/data-macro qualification remains pending.
    pub fn preview_ace16_split_int32_updates(
        self,
        table: &str,
        column: &str,
        updates: &[Int32Update],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.split_int32",
            assignments = updates.len()
        )
        .entered();
        let operation = Operation {
            table,
            column,
            requests: updates,
        };
        let staged = stage(&self, &operation, budget, cancelled)?;
        finish::finish(self, &operation, &staged, budget, cancelled)
    }
}

struct Operation<'a> {
    table: &'a str,
    column: &'a str,
    requests: &'a [Int32Update],
}

struct Staged<const WIDTH: usize = { size_of::<i32>() }> {
    structural: TableCheck,
    patches: Vec<Patch<WIDTH>>,
    plan: SplitIndexPlan,
}

fn stage(
    image: &NativeImage,
    operation: &Operation<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let structural = preflight(image, operation, budget, cancelled)?;
    stage_checked(image, operation, structural, budget, cancelled)
}

fn preflight(
    image: &NativeImage,
    operation: &impl Scope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    check(budget, cancelled)?;
    preview::qualify(image)?;
    if operation.request_count() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    operation.unique(budget, cancelled)?;
    gates::before(image, operation.table(), budget, cancelled)
}

fn stage_checked<const WIDTH: usize>(
    image: &NativeImage,
    operation: &impl Binding<WIDTH>,
    structural: TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged<WIDTH>, EditError> {
    let schema = structural.schema();
    let (position, selected_kind) = binding::position(operation, schema)?;
    let mut source = image.reader();
    let patches = operation.patches(
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        },
        schema,
        position,
    )?;
    let indexed = structural.indexes().iter().any(|index| {
        index
            .index()
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[position].number())
    });
    let mut keys = Vec::new();
    if indexed {
        budget
            .charge_allocation(patches.len() as u64, size_of::<IndexKeyUpdate>() as u64)
            .map_err(InspectError::from)?;
        keys.try_reserve_exact(patches.len())
            .map_err(|_| InspectError::Allocation)?;
        let kind = MutationKey::new(selected_kind)?;
        for patch in &patches {
            check(budget, cancelled)?;
            keys.push(IndexKeyUpdate {
                reference: patch.reference,
                column: position,
                expected: Some(kind.stored(&patch.before)?),
                value: Some(kind.stored(&patch.after)?),
            });
        }
    }
    let plan = index_split_patches(
        image,
        schema,
        structural.indexes(),
        &keys,
        budget,
        cancelled,
    )?;
    Ok(Staged {
        structural,
        patches,
        plan,
    })
}
