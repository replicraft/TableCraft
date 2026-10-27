//! Mutable map rows and bitmap storage need authority even without data members.
use super::super::super::{EditError, WriteBlocker};
use crate::{
    InspectError,
    format::{
        allocation::{MapRole, MapStorage, RepresentedMap},
        reader::Reader,
        row::reference_page,
    },
    header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

// Global plus ordinary and selected-column owned/candidate pairs.
const MUTABLE_ROLES: usize = 5;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    GlobalFree,
    OrdinaryOwned,
    OrdinaryCandidate,
    IndexOwned(u32),
    ColumnOwned(usize),
    ColumnCandidate(usize),
}

impl Role {
    fn grammar(self) -> MapRole {
        match self {
            Self::GlobalFree => MapRole::GlobalFree,
            Self::OrdinaryCandidate => MapRole::TableFreeSpace,
            Self::ColumnCandidate(_) => MapRole::LongValueCandidate,
            Self::OrdinaryOwned | Self::IndexOwned(_) | Self::ColumnOwned(_) => MapRole::Owned,
        }
    }

    fn mutable(self, definition: u32, selected: u32, column: usize) -> bool {
        match self {
            Self::GlobalFree => true,
            Self::OrdinaryOwned | Self::OrdinaryCandidate => definition == selected,
            Self::ColumnOwned(number) | Self::ColumnCandidate(number) => {
                definition == selected && number == column
            }
            Self::IndexOwned(_) => false,
        }
    }
}

struct Claim {
    definition: u32,
    role: Role,
    reference: u32,
    // The first page is the container; remaining pages are bitmap storage.
    storage: MapStorage,
}

pub(super) struct Census {
    claims: Vec<Claim>,
}

impl Census {
    pub fn new() -> Self {
        Self { claims: Vec::new() }
    }

    pub fn capture(
        &mut self,
        definition: u32,
        role: Role,
        reference: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<&[u32], EditError> {
        reader.tick()?;
        reader.work(self.claims.len())?;
        if let Some(position) = self
            .claims
            .iter()
            .position(|claim| claim.definition == definition && claim.role == role)
        {
            // Logical aliases of one physical index have one canonical owner.
            // Conflicting references for that owner are malformed metadata.
            let claim = &self.claims[position];
            if claim.reference != reference {
                return Err(InspectError::Malformed.into());
            }

            return Ok(&claim.storage.pages);
        }

        let storage = RepresentedMap::read_storage(reader, reference, role.grammar())?;
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
            definition,
            role,
            reference,
            storage,
        });
        let last = self.claims.last().ok_or(InspectError::Malformed)?;

        Ok(&last.storage.pages)
    }

    pub fn require_exclusive(
        &self,
        definition: u32,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let mut mutable = 0;
        for claim in &self.claims {
            check(budget, cancelled)?;
            if !claim.role.mutable(claim.definition, definition, column) {
                continue;
            }

            mutable += 1;
            for other in &self.claims {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                if claim.definition == other.definition && claim.role == other.role {
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
        if mutable != MUTABLE_ROLES {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        Ok(())
    }
}
