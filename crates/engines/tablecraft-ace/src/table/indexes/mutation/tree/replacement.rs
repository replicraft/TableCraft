//! Equal-cardinality replacement of owned keys, retaining original topology.
use super::super::{
    IndexDataPatch, repack,
    retained::{self, RetainedProfile},
};
use super::{Tree, raw_page};
use crate::table::indexes::{
    key::KeyWork,
    page::{Entry, KeyRef},
};
use crate::{EditError, InspectError, WriteBlocker, header::check};
use tablecraft_core::limits::ResourceBudget;

impl Tree {
    pub(in crate::table::indexes::mutation) fn replace<P: RetainedProfile, const N: usize>(
        &self,
        image: &crate::NativeImage,
        keys: &mut &[KeyRef<P::Key>],
        profile: &P,
        patches: &mut Vec<IndexDataPatch>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<KeyRef<P::Key>>, EditError> {
        check(budget, cancelled)?;
        budget
            .charge_allocation(self.count as u64, size_of::<Entry<P::Key>>() as u64)
            .map_err(InspectError::from)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.count)
            .map_err(|_| InspectError::Allocation)?;

        let last = if self.children.is_empty() {
            let (slice, rest) = keys
                .split_at_checked(self.count)
                .ok_or(EditError::Preservation)?;
            for value in slice {
                check(budget, cancelled)?;
                entries.push(Entry {
                    value: retained::reference(profile, value, &mut KeyWork { budget, cancelled })?,
                    child: 0,
                });
            }
            *keys = rest;
            slice
                .last()
                .map(|value| {
                    retained::reference(profile, value, &mut KeyWork { budget, cancelled })
                })
                .transpose()?
        } else {
            let mut last = None;
            for (position, child) in self.children.iter().enumerate() {
                check(budget, cancelled)?;
                let maximum = child
                    .replace::<_, N>(image, keys, profile, patches, budget, cancelled)?
                    .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
                if position < self.count {
                    entries.push(Entry {
                        value: retained::reference(
                            profile,
                            &maximum,
                            &mut KeyWork { budget, cancelled },
                        )?,
                        child: child.number,
                    });
                }
                last = Some(maximum);
            }
            last
        };

        let raw = raw_page(image, self.number)?;
        let after = repack::retained_page::<_, N>(raw, &entries, profile, budget, cancelled)?;
        repack::patches(raw, &after, self.number, patches, budget, cancelled)?;
        Ok(last)
    }
}
