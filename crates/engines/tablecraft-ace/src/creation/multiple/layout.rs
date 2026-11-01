//! Checked complete-image geometry and explicit stable append-map roles.
use super::super::{CreateDatabaseError, profile, request::Request};
use crate::format::{page::PAGE_BYTES, row};
use crate::header::check;
use tablecraft_core::limits::ResourceBudget;

pub(super) const MAX_TABLES: usize = 16;
const USER_PAGES: usize = 2;
pub(super) const APPENDED_MAPS: usize = 2;

pub(super) struct Layout {
    tables: usize,
    pub image_bytes: usize,
    pub map_rows: usize,
    pub catalog_rows: usize,
    indexes: [Option<IndexRole>; MAX_TABLES],
}

pub(super) struct Role {
    pub definition: usize,
    pub data: usize,
    pub owned: usize,
    pub candidate: usize,
}

#[derive(Clone, Copy)]
pub(super) struct IndexRole {
    pub root: usize,
    pub owned: usize,
}

impl IndexRole {
    pub fn owned_reference(self) -> Result<u32, CreateDatabaseError> {
        reference(self.owned)
    }
}

impl Role {
    pub fn owned_reference(&self) -> Result<u32, CreateDatabaseError> {
        reference(self.owned)
    }

    pub fn candidate_reference(&self) -> Result<u32, CreateDatabaseError> {
        reference(self.candidate)
    }
}

impl Layout {
    pub fn capture(tables: usize) -> Result<Self, CreateDatabaseError> {
        if !(1..=MAX_TABLES).contains(&tables) {
            return Err(CreateDatabaseError::TableLimit);
        }
        let pages = tables
            .checked_mul(USER_PAGES)
            .and_then(|pages| pages.checked_add(profile::USER_DEFINITION))
            .ok_or(CreateDatabaseError::Layout)?;
        if pages > profile::MAP_BITMAP_BYTES * u8::BITS as usize {
            return Err(CreateDatabaseError::Layout);
        }
        let image_bytes = pages
            .checked_mul(PAGE_BYTES)
            .ok_or(CreateDatabaseError::Layout)?;
        let map_rows = (tables - 1)
            .checked_mul(APPENDED_MAPS)
            .and_then(|rows| rows.checked_add(profile::MAP_ROWS))
            .ok_or(CreateDatabaseError::Layout)?;
        reference(map_rows - 1)?;
        let map_bytes = map_rows
            .checked_mul(profile::MAP_RECORD_BYTES + row::SLOT_BYTES)
            .and_then(|bytes| bytes.checked_add(super::super::record::DATA_HEADER_BYTES))
            .ok_or(CreateDatabaseError::Layout)?;
        if map_bytes > PAGE_BYTES {
            return Err(CreateDatabaseError::PageCapacity);
        }
        let catalog_rows = tables
            .checked_add(profile::SYSTEM_OBJECT_COUNT)
            .ok_or(CreateDatabaseError::Layout)?;
        Ok(Self {
            tables,
            image_bytes,
            map_rows,
            catalog_rows,
            indexes: [None; MAX_TABLES],
        })
    }

    /// Add exactly one page/map for each actual validated primary request.
    /// Existing user TD/DATA and owned/candidate identities remain stable.
    pub fn bind_primary(
        mut self,
        requests: &[Request<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        if requests.len() != self.tables {
            return Err(CreateDatabaseError::Layout);
        }
        let first_root = self.image_bytes / PAGE_BYTES;
        let first_map = self.map_rows;
        let mut added = 0;
        for (index, request) in requests.iter().enumerate() {
            check(budget, cancelled)?;
            if request.primary.is_some() {
                self.indexes[index] = Some(IndexRole {
                    root: first_root + added,
                    owned: first_map + added,
                });
                added += 1;
            }
        }
        let pages = first_root
            .checked_add(added)
            .ok_or(CreateDatabaseError::Layout)?;
        if pages > profile::MAP_BITMAP_BYTES * u8::BITS as usize {
            return Err(CreateDatabaseError::PageCapacity);
        }
        self.image_bytes = pages
            .checked_mul(PAGE_BYTES)
            .ok_or(CreateDatabaseError::Layout)?;
        self.map_rows = first_map
            .checked_add(added)
            .ok_or(CreateDatabaseError::Layout)?;
        reference(self.map_rows - 1)?;
        let map_bytes = self
            .map_rows
            .checked_mul(profile::MAP_RECORD_BYTES + row::SLOT_BYTES)
            .and_then(|bytes| bytes.checked_add(super::super::record::DATA_HEADER_BYTES))
            .ok_or(CreateDatabaseError::PageCapacity)?;
        if map_bytes > PAGE_BYTES {
            return Err(CreateDatabaseError::PageCapacity);
        }
        check(budget, cancelled)?;
        Ok(self)
    }

    pub fn index_role(&self, index: usize) -> Result<Option<IndexRole>, CreateDatabaseError> {
        if index >= self.tables {
            return Err(CreateDatabaseError::Layout);
        }
        Ok(self.indexes[index])
    }

    pub fn index_maps_start(&self) -> usize {
        profile::MAP_ROWS + (self.tables - 1) * APPENDED_MAPS
    }

    pub fn role(&self, index: usize) -> Result<Role, CreateDatabaseError> {
        if index >= self.tables {
            return Err(CreateDatabaseError::Layout);
        }
        let definition = index
            .checked_mul(USER_PAGES)
            .and_then(|pages| pages.checked_add(profile::USER_DEFINITION))
            .ok_or(CreateDatabaseError::Layout)?;
        let data = definition
            .checked_add(1)
            .ok_or(CreateDatabaseError::Layout)?;
        let (owned, candidate) = if index == 0 {
            (profile::USER_MAP_SLOT, profile::USER_CANDIDATE_SLOT)
        } else {
            let owned = (index - 1)
                .checked_mul(APPENDED_MAPS)
                .and_then(|slot| slot.checked_add(profile::MAP_ROWS))
                .ok_or(CreateDatabaseError::Layout)?;
            (
                owned,
                owned.checked_add(1).ok_or(CreateDatabaseError::Layout)?,
            )
        };
        Ok(Role {
            definition,
            data,
            owned,
            candidate,
        })
    }
}

fn reference(slot: usize) -> Result<u32, CreateDatabaseError> {
    let slot = u8::try_from(slot).map_err(|_| CreateDatabaseError::Layout)?;
    let page = u32::try_from(profile::MAP_PAGE).map_err(|_| CreateDatabaseError::Layout)?;
    Ok(row::reference(page, slot))
}
