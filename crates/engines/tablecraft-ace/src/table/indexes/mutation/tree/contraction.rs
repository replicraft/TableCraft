//! Empty branches retire; nonempty nodes keep height and original child form.
use super::{Tree, raw_page};
use crate::table::indexes::{
    NativeIndex,
    header::{IndexHeader, LEVEL_OFFSET},
    mutation::{
        IndexDataPatch,
        profile::WriteProfile,
        repack,
        retirement::{RetiredPage, header_patch},
    },
    page::{Entry, KeyRef, LEAF},
};
use crate::{EditError, InspectError, NativeImage, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::indexes::mutation) struct Contraction<'a, P> {
    pub profile: &'a P,
    pub references: &'a [u32],
    pub index: &'a NativeIndex,
    pub retired: &'a mut Vec<RetiredPage>,
    pub leaves: Vec<u32>,
    pub removed: usize,
}

impl Tree {
    pub(in crate::table::indexes::mutation) fn needs_contraction<K>(
        &self,
        keys: &mut &[KeyRef<K>],
        references: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        if self.children.is_empty() {
            let (original, rest) = keys
                .split_at_checked(self.count)
                .ok_or(EditError::Preservation)?;
            *keys = rest;
            for key in original {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, references.len() as u64)
                    .map_err(InspectError::from)?;
                if !references.contains(&key.row) {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        let mut needed = false;
        for child in &self.children {
            needed |= child.needs_contraction(keys, references, budget, cancelled)?;
        }
        Ok(needed)
    }

    pub(in crate::table::indexes::mutation) fn contract<P: WriteProfile, const N: usize>(
        &self,
        image: &NativeImage,
        keys: &mut &[KeyRef<P::Key>],
        state: &mut Contraction<'_, P>,
        patches: &mut Vec<IndexDataPatch>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<KeyRef<P::Key>>, EditError> {
        check(budget, cancelled)?;
        let raw = raw_page(image, self.number)?;
        let header = IndexHeader::read(raw)?;
        if !self.children.is_empty() && (header.previous.get() != 0 || header.next.get() != 0) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        budget
            .charge_allocation(self.count as u64, size_of::<Entry<P::Key>>() as u64)
            .map_err(InspectError::from)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.count)
            .map_err(|_| InspectError::Allocation)?;
        let mut tail = 0;
        let last = if self.children.is_empty() {
            let (original, rest) = keys
                .split_at_checked(self.count)
                .ok_or(EditError::Preservation)?;
            *keys = rest;
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
                        value: *key,
                        child: 0,
                    });
                }
            }
            entries.last().map(|entry| entry.value)
        } else {
            let tail_form = header.tail.get() != 0;
            let mut pending = None;
            for child in &self.children {
                let maximum =
                    child.contract::<P, N>(image, keys, state, patches, budget, cancelled)?;
                if let Some(value) = maximum {
                    if tail_form {
                        if let Some((previous, number)) = pending {
                            entries.push(Entry {
                                value: previous,
                                child: number,
                            });
                        }
                        pending = Some((value, child.number));
                    } else {
                        entries.push(Entry {
                            value,
                            child: child.number,
                        });
                        pending = Some((value, child.number));
                    }
                }
            }
            if tail_form {
                tail = pending.map_or(0, |(_, number)| number);
            }
            pending.map(|(value, _)| value)
        };
        let root = self.number == state.index.root_page();
        if last.is_none() && !root {
            budget
                .charge_allocation(1, size_of::<RetiredPage>() as u64)
                .map_err(InspectError::from)?;
            state
                .retired
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            state.retired.push(RetiredPage {
                number: self.number,
                physical: state.index.data_number(),
                usage: state.index.usage_map(),
                root: state.index.root_page(),
            });
            return Ok(None);
        }
        let after = repack::page::<P, N>(raw, &entries, state.profile, budget, cancelled)?;
        repack::patches(raw, &after, self.number, patches, budget, cancelled)?;
        if self.children.is_empty() || last.is_none() {
            budget
                .charge_allocation(1, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            state
                .leaves
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            state.leaves.push(self.number);
        }
        if last.is_none() {
            header_patch(
                image,
                self.number,
                std::mem::offset_of!(IndexHeader, kind),
                &[LEAF],
                patches,
                budget,
                cancelled,
            )?;
            header_patch(
                image,
                self.number,
                LEVEL_OFFSET,
                &[0],
                patches,
                budget,
                cancelled,
            )?;
            tail = 0;
        }
        if !self.children.is_empty() {
            header_patch(
                image,
                self.number,
                std::mem::offset_of!(IndexHeader, tail),
                &tail.to_le_bytes(),
                patches,
                budget,
                cancelled,
            )?;
        }
        Ok(last)
    }
}
