//! Checked complete-image geometry and explicit stable append-map roles.
use super::super::{CreateDatabaseError, profile};
use crate::format::{page::PAGE_BYTES, row};

pub(super) const MAX_TABLES: usize = 16;
const USER_PAGES: usize = 2;
pub(super) const APPENDED_MAPS: usize = 2;

pub(super) struct Layout {
    tables: usize,
    pub image_bytes: usize,
    pub map_rows: usize,
    pub catalog_rows: usize,
}

pub(super) struct Role {
    pub definition: usize,
    pub data: usize,
    pub owned: usize,
    pub candidate: usize,
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
        })
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
