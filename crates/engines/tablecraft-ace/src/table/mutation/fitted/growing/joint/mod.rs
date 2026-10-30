//! One original mixed record operation continues through DATA and INDEX growth.
pub(in crate::table) mod certified;
mod finish;

use super::super::request::{self, FittedColumnUpdates};
use super::stage;
use crate::{
    Ace16WritePreview, EditError, NativeImage,
    table::{
        indexes::{
            SplitIndexPlan,
            mutation::split::rows::{self, Changes},
        },
        mutation::{preview, variable_update::placement::Data},
    },
};
use tablecraft_core::limits::ResourceBudget;

struct Staged {
    data: Data,
    indexes: SplitIndexPlan,
    changed: usize,
}

impl NativeImage {
    /// Preview fixed and one Short Text/Binary field jointly with DATA placement
    /// and qualified INDEX splitting under one append cursor. Original groups
    /// independently certify complete records, keys, ownership and map effects.
    ///
    /// This separate engineering candidate does not enable normal ACE saves,
    /// data macros, or native Access qualification.
    pub fn preview_ace16_mixed_split_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!("native.edit.mixed_split_preview").entered();
        request::check(budget, cancelled)?;
        preview::qualify(&self)?;

        let staged = prepare(&self, table, groups, budget, cancelled)?;

        finish::finish(self, table, groups, &staged, budget, cancelled)
    }
}

fn prepare(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let prepared = stage::prepare_with(
        image,
        table,
        groups,
        stage::Continuation::JointIndexes,
        budget,
        cancelled,
    )?;
    let keys = prepared.verified.keys(budget, cancelled)?;
    let changes = Changes {
        columns: &prepared.verified.columns,
        updates: &keys,
    };
    let (data, append) = prepared.placement.into_parts();
    let indexes = rows::continue_patches(
        image,
        prepared.verified.checked.schema(),
        prepared.verified.checked.indexes(),
        &changes,
        append,
        budget,
        cancelled,
    )?;
    Ok(Staged {
        data,
        indexes,
        changed: prepared.verified.records.len(),
    })
}

#[cfg(test)]
mod tests;
