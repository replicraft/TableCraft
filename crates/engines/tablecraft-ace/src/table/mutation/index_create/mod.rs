//! Source-bound creation of one real ordinary Int32 secondary index.
mod audit;
pub(in crate::table) mod buffer;
#[cfg(test)]
mod generated_sources;
mod layout;
mod model;
pub(in crate::table) mod source;
mod stage;

use crate::{EditError, NativeImage, header::check, table::mutation::preview};
use tablecraft_core::limits::ResourceBudget;

pub use model::{Ace16IndexCreatePreview, SecondaryIndexCreate};

impl NativeImage {
    /// Build a separate engineering candidate containing a new physical index.
    ///
    /// The initial profile admits one ordinary nonunique Int32 secondary index,
    /// including NULL keys, on a previously unindexed field. It requires a
    /// qualified single-page definition and represented inline allocation maps.
    /// This candidate grants no normal-save or required-macro authority.
    pub fn preview_ace16_create_secondary_index(
        self,
        table: &str,
        request: &SecondaryIndexCreate<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16IndexCreatePreview, EditError> {
        let _span = tracing::debug_span!("native.edit.preview.create_secondary_index").entered();
        preview::qualify(&self)?;
        check(budget, cancelled)?;

        let original = source::qualify(&self, table, request, budget, cancelled)?;
        let plan = stage::prepare(&self, table, request, &original, budget, cancelled)?;
        let after = stage::compose(&self, &original, &plan, budget, cancelled)?;

        // Original caller intent remains an audit input, independently of the
        // prepared population, descriptor, allocator ledger and emitted pages.
        let indexed_rows = audit::prove(&self, table, request, &plan, &after, budget, cancelled)?;
        check(budget, cancelled)?;

        Ok(Ace16IndexCreatePreview {
            before: self,
            after,
            name: original.name,
            logical: original.layout.logical_count,
            physical: original.layout.physical_count,
            indexed_rows,
        })
    }
}
