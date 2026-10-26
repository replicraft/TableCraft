//! Retain original leaf partitions and topology while removing exact references.
use super::{Tree, raw_page};
use crate::table::indexes::{
    key::KeyWork,
    mutation::{
        IndexDataPatch, repack,
        retained::{self, RetainedProfile},
    },
    page::{Entry, KeyRef},
};
use crate::{EditError, InspectError, NativeImage, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::indexes::mutation) struct Deletion<'a, P> {
    pub profile: &'a P,
    pub references: &'a [u32],
    pub removed: usize,
}

impl Tree {
    pub(in crate::table::indexes::mutation) fn delete<P: RetainedProfile, const N: usize>(
        &self,
        image: &NativeImage,
        keys: &mut &[KeyRef<P::Key>],
        state: &mut Deletion<'_, P>,
        patches: &mut Vec<IndexDataPatch>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<KeyRef<P::Key>, EditError> {
        check(budget, cancelled)?;
        budget
            .charge_allocation(self.count as u64, size_of::<Entry<P::Key>>() as u64)
            .map_err(InspectError::from)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.count)
            .map_err(|_| InspectError::Allocation)?;
        let last = if self.children.is_empty() {
            // Consume the ORIGINAL population even when this leaf loses entries.
            // A globally filtered stream would incorrectly shift later leaves.
            let (original, remaining) = keys
                .split_at_checked(self.count)
                .ok_or(EditError::Preservation)?;
            *keys = remaining;
            for key in original {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, state.references.len() as u64)
                    .map_err(InspectError::from)?;
                if state.references.contains(&key.row) {
                    state.removed = state
                        .removed
                        .checked_add(1)
                        .ok_or(InspectError::StructureLimit)?;
                } else {
                    entries.push(Entry {
                        value: retained::reference(
                            state.profile,
                            key,
                            &mut KeyWork { budget, cancelled },
                        )?,
                        child: 0,
                    });
                }
            }
            entries
                .last()
                .map(|entry| {
                    retained::reference(
                        state.profile,
                        &entry.value,
                        &mut KeyWork { budget, cancelled },
                    )
                })
                .transpose()?
                .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?
        } else {
            let mut last = None;
            for (position, child) in self.children.iter().enumerate() {
                check(budget, cancelled)?;
                let maximum =
                    child.delete::<P, N>(image, keys, state, patches, budget, cancelled)?;
                if position < self.count {
                    // Full key AND row reference identify boundaries for ties,
                    // including repeated NULL keys in adjacent child subtrees.
                    entries.push(Entry {
                        value: retained::reference(
                            state.profile,
                            &maximum,
                            &mut KeyWork { budget, cancelled },
                        )?,
                        child: child.number,
                    });
                }
                last = Some(maximum);
            }
            last.ok_or(EditError::Preservation)?
        };
        let original = raw_page(image, self.number)?;
        let after =
            repack::retained_page::<P, N>(original, &entries, state.profile, budget, cancelled)?;
        repack::patches(original, &after, self.number, patches, budget, cancelled)?;
        Ok(last)
    }
}
