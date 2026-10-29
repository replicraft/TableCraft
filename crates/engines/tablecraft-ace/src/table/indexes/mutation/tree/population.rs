//! Fitting cardinality changes retain every original page, link and height.
use super::{Tree, raw_page};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    header::check,
    table::indexes::{
        mutation::{IndexDataPatch, profile::WriteProfile, repack},
        page::{Entry, KeyRef},
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table::indexes::mutation) struct Population<'a, K> {
    pub keys: &'a [KeyRef<K>],
    pub leaves: usize,
}

impl Tree {
    pub(in crate::table::indexes::mutation) fn leaf_count(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        check(budget, cancelled)?;
        if self.children.is_empty() {
            return Ok(1);
        }
        let mut count = 0_usize;
        for child in &self.children {
            count = count
                .checked_add(child.leaf_count(budget, cancelled)?)
                .ok_or(InspectError::StructureLimit)?;
        }
        Ok(count)
    }

    pub(in crate::table::indexes::mutation) fn rewrite_population<
        P: WriteProfile,
        const N: usize,
    >(
        &self,
        image: &NativeImage,
        population: &mut Population<'_, P::Key>,
        profile: &P,
        patches: &mut Vec<IndexDataPatch>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<KeyRef<P::Key>>, EditError> {
        check(budget, cancelled)?;
        let capacity = if self.children.is_empty() {
            if population.leaves == 0 {
                return Err(EditError::Preservation);
            }
            let reserved_for_later = population.leaves - 1;
            let available = population
                .keys
                .len()
                .checked_sub(reserved_for_later)
                .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
            if reserved_for_later == 0 {
                available
            } else {
                self.count.min(available)
            }
        } else {
            self.count
        };
        budget
            .charge_allocation(capacity as u64, size_of::<Entry<P::Key>>() as u64)
            .map_err(InspectError::from)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| InspectError::Allocation)?;
        let last = if self.children.is_empty() {
            let (assigned, rest) = population
                .keys
                .split_at_checked(capacity)
                .ok_or(EditError::Preservation)?;
            for value in assigned {
                check(budget, cancelled)?;
                entries.push(Entry {
                    value: *value,
                    child: 0,
                });
            }
            population.keys = rest;
            population.leaves -= 1;
            assigned.last().copied()
        } else {
            let mut last = None;
            for (position, child) in self.children.iter().enumerate() {
                let maximum = child
                    .rewrite_population::<_, N>(
                        image, population, profile, patches, budget, cancelled,
                    )?
                    .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
                if position < self.count {
                    entries.push(Entry {
                        value: maximum,
                        child: child.number,
                    });
                }
                last = Some(maximum);
            }
            last
        };
        let original = raw_page(image, self.number)?;
        let after = repack::page::<_, N>(original, &entries, profile, budget, cancelled)?;
        repack::patches(original, &after, self.number, patches, budget, cancelled)?;
        Ok(last)
    }
}
