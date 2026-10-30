//! Mixed complete-row placement with the existing represented allocation policy.
mod finish;
pub(in crate::table) mod joint;
mod stage;

use super::request::{self, FittedColumnUpdates};
use crate::{
    Ace16WritePreview, EditError, NativeImage,
    table::{indexes::index_row_patches, mutation::preview},
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Preview fixed and one ordinary Short Text/Binary field jointly, allowing
    /// existing-page hidden targets or checked EOF DATA allocation. INDEX pages
    /// must fit. Logical row identities and original expectations remain stable.
    /// This engineering candidate does not enable normal saves or data macros.
    pub fn preview_ace16_mixed_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!("native.edit.mixed_preview").entered();
        request::check(budget, cancelled)?;
        preview::qualify(&self)?;
        let prepared = stage::prepare(&self, table, groups, budget, cancelled)?;
        let keys = prepared.verified.keys(budget, cancelled)?;
        let indexes = index_row_patches(
            &self,
            prepared.verified.checked.schema(),
            prepared.verified.checked.indexes(),
            &prepared.verified.columns,
            &keys,
            budget,
            cancelled,
        )?;
        finish::finish(self, table, groups, &prepared, &indexes, budget, cancelled)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod joint_split_tests;
