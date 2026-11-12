//! Complete normal index removal, separate from row-update retirement.
mod definition;
mod effect;
pub(crate) use effect::{NativeIndexDropEffect, NativeIndexDropParts};
mod proof;
mod retirement;
mod source;
mod unique;
pub use unique::NativeUniqueLongIndexDropCandidate;
#[cfg(test)]
mod tests;

use super::index_creation::buffer;
use crate::{EditError, InspectError, NativeImage, NativeImageSource};
use tablecraft_core::limits::ResourceBudget;

/// Independently proved ordinary LONG index removal, without persistence authority.
///
/// Exclusive retired trees and their dedicated map storage are zeroed. This is
/// an explicit producer policy, not observed Microsoft Access byte parity.
pub struct NativeIndexDropCandidate {
    policy: source::Policy,
    before: NativeImage,
    after: NativeImage,
    table: String,
    index: String,
    native_table: i32,
    definition: u32,
    logical: u32,
    physical: u32,
    retired_pages: usize,
}

impl NativeIndexDropCandidate {
    /// Complete original image bound to this candidate.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }
    /// Complete checked replacement, without mutable storage or journal authority.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }
    /// Requested table spelling.
    pub fn table(&self) -> &str {
        &self.table
    }
    /// Requested logical index spelling.
    pub fn index_name(&self) -> &str {
        &self.index
    }
    /// Original signed catalog table identity.
    pub const fn native_table_id(&self) -> i32 {
        self.native_table
    }
    /// Original table-definition root.
    pub const fn definition_page(&self) -> u32 {
        self.definition
    }
    /// Removed logical index identity, separate from descriptor position.
    pub const fn logical_index_number(&self) -> u32 {
        self.logical
    }
    /// Original physical tree identity; a surviving alias retains its tree.
    pub const fn physical_index_number(&self) -> u32 {
        self.physical
    }
    /// Reclaimed tree and dedicated map pages; zero when another alias survives.
    pub const fn retired_pages(&self) -> usize {
        self.retired_pages
    }
}

impl NativeImage {
    /// Remove a qualified ordinary nonunique LONG index from a retained table.
    ///
    /// Complete source, ownership and membership certificates precede any effect.
    /// DATA, LVAL, catalog chronology and generated counters are preserved. The
    /// candidate does not grant save authority or establish native application parity.
    ///
    /// # Errors
    /// Refuses unsupported declarations or dependencies, shared map containers
    /// whose retirement is not qualified, malformed ownership/membership, allocation
    /// failures, cancellation, exhausted caller budgets or a failed independent proof.
    pub fn prepare_retained_index_drop(
        self,
        table: &str,
        index: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeIndexDropCandidate, EditError> {
        self.build_retained_index_drop(table, index, source::Policy::Ordinary, budget, cancelled)
    }

    fn build_retained_index_drop(
        self,
        table: &str,
        index: &str,
        policy: source::Policy,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeIndexDropCandidate, EditError> {
        let _span = tracing::debug_span!("native.index.drop").entered();
        let original = match policy {
            source::Policy::Ordinary => source::capture(&self, table, index, budget, cancelled)?,
            source::Policy::UniqueLong => {
                source::capture_policy(&self, table, index, policy, budget, cancelled)?
            }
        };
        let mut bytes = buffer::copy(self.as_bytes(), budget, cancelled)?;
        let page = definition::compose(&self, &original, budget, cancelled)?;
        let start = original.layout.definition as usize * crate::format::page::PAGE_BYTES;
        bytes[start..start + page.len()].copy_from_slice(&page);
        retirement::apply(&self, &original.retired, &mut bytes, budget, cancelled)?;
        let after = NativeImage::capture_shared_retained(
            NativeImageSource::from_vec(bytes, budget, cancelled)?,
            self.catalog().catalog().scope(),
            budget,
            cancelled,
        )?;
        match policy {
            source::Policy::Ordinary => {
                proof::prove(&self, &after, table, index, budget, cancelled)?
            }
            source::Policy::UniqueLong => {
                proof::prove_policy(&self, &after, table, index, policy, budget, cancelled)?
            }
        }
        let table = String::from_utf8(buffer::copy(table.as_bytes(), budget, cancelled)?)
            .map_err(|_| InspectError::Malformed)?;
        let index = String::from_utf8(buffer::copy(index.as_bytes(), budget, cancelled)?)
            .map_err(|_| InspectError::Malformed)?;
        let selected = original.selected();
        let logical = selected.number();
        let physical = selected.data_number();
        let retired_pages = original.retired.len();
        let definition = original.layout.definition;
        let native_table = original.native_table;
        Ok(NativeIndexDropCandidate {
            policy,
            before: self,
            after,
            table,
            index,
            native_table,
            definition,
            logical,
            physical,
            retired_pages,
        })
    }
}
