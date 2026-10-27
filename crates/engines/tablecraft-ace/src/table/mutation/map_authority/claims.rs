//! Exact semantic owner claims and protected storage footprints.
use super::Owner;
use crate::{
    InspectError,
    format::{
        allocation::{MapStorage, RepresentedMap},
        reader::Reader,
        row::reference_page,
    },
    header::check,
    table::mutation::{EditError, WriteBlocker},
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

struct Claim {
    owner: Owner,
    reference: u32,
    // The first page is the container; remaining pages are bitmap storage.
    storage: MapStorage,
}

pub(in crate::table) struct Census {
    claims: Vec<Claim>,
}

/// Requested original storage identity; constructing it grants no authority.
pub(in crate::table) struct RecordIdentity {
    pub owner: Owner,
    pub reference: u32,
    pub range: Range<usize>,
}

impl Census {
    pub(super) fn new() -> Self {
        Self { claims: Vec::new() }
    }

    pub(super) fn capture(
        &mut self,
        owner: Owner,
        reference: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<&[u32], EditError> {
        reader.tick()?;
        reader.work(self.claims.len())?;
        if let Some(position) = self.claims.iter().position(|claim| claim.owner == owner) {
            // Logical aliases of one physical index have one canonical owner.
            // Conflicting references for that owner are malformed metadata.
            let claim = &self.claims[position];
            if claim.reference != reference {
                return Err(InspectError::Malformed.into());
            }

            return Ok(&claim.storage.pages);
        }

        let storage = RepresentedMap::read_storage(reader, reference, owner.grammar())?;
        if storage.pages.first().copied() != Some(reference_page(reference)) {
            return Err(InspectError::Malformed.into());
        }
        reader
            .budget
            .charge_allocation(1, size_of::<Claim>() as u64)
            .map_err(InspectError::from)?;
        self.claims
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.claims.push(Claim {
            owner,
            reference,
            storage,
        });
        let last = self.claims.last().ok_or(InspectError::Malformed)?;

        Ok(&last.storage.pages)
    }

    pub(in crate::table) fn protect_storage(
        &self,
        protected: &mut Vec<u32>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        for claim in &self.claims {
            for &page in &claim.storage.pages {
                reader.tick()?;
                reader.work(protected.len())?;
                if protected.contains(&page) {
                    continue;
                }
                reader
                    .budget
                    .charge_allocation(1, size_of::<u32>() as u64)
                    .map_err(InspectError::from)?;
                protected
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                protected.push(page);
            }
        }

        Ok(())
    }

    pub fn require_exclusive(
        &self,
        selected: &[Owner],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for (position, owner) in selected.iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (position + self.claims.len()) as u64)
                .map_err(InspectError::from)?;
            if selected[..position].contains(owner) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let claim = self
                .claims
                .iter()
                .find(|claim| claim.owner == *owner)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            for other in &self.claims {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                if claim.owner == other.owner {
                    continue;
                }
                if claim.reference == other.reference {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                if claim.storage.record.start < other.storage.record.end
                    && other.storage.record.start < claim.storage.record.end
                {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }

                // Distinct rows may share their ordinary container. A bitmap
                // page cannot simultaneously be another owner's map storage.
                for &bitmap in &claim.storage.pages[1..] {
                    check(budget, cancelled)?;
                    budget
                        .charge(Resource::Steps, other.storage.pages.len() as u64)
                        .map_err(InspectError::from)?;
                    if other.storage.pages.contains(&bitmap) {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                }
                for &bitmap in &other.storage.pages[1..] {
                    check(budget, cancelled)?;
                    budget
                        .charge(Resource::Steps, 1)
                        .map_err(InspectError::from)?;
                    if bitmap == claim.storage.pages[0] {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                }
            }
        }

        Ok(())
    }

    /// Bind a complete original record to its canonical mutable owner.
    /// A requested identity alone does not prove role or storage exclusivity.
    pub(in crate::table) fn require_record_rewrite(
        &self,
        selected: &[Owner],
        record: RecordIdentity,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        self.require_exclusive(selected, budget, cancelled)?;
        budget
            .charge(Resource::Steps, (selected.len() + self.claims.len()) as u64)
            .map_err(InspectError::from)?;
        if !selected.contains(&record.owner) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        let claim = self
            .claims
            .iter()
            .find(|claim| claim.owner == record.owner)
            .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
        if claim.reference != record.reference || claim.storage.record != record.range {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        Ok(())
    }
}
