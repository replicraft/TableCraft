//! Bounded append roles; file length is framing, never ownership evidence.
use crate::format::page::PAGE_BYTES;

const SYSTEM_PAGES: usize = 7;
const TABLE_PAGES: usize = 3;
const SYSTEM_OBJECTS: usize = 7;
const RESERVED_MAPS: usize = 20;
const MAP_BITMAP_BYTES: usize = 8;
const MAPPED_PAGES: usize = MAP_BITMAP_BYTES * u8::BITS as usize;
const FIRST_OWNED_SLOT: usize = 3;
const FIRST_CANDIDATE_SLOT: usize = 7;
const FIRST_INDEX_SLOT: usize = 22;
const LATER_OWNED_BASE: usize = 20;

#[derive(Clone, Copy)]
pub(super) struct Role {
    pub definition: usize,
    pub data: usize,
    pub root: usize,
    pub owned: usize,
    pub candidate: usize,
    pub index: usize,
    pub catalog: usize,
}

impl Role {
    fn at(table: usize) -> Option<Self> {
        let definition = SYSTEM_PAGES.checked_add(table.checked_mul(TABLE_PAGES)?)?;
        let data = definition.checked_add(1)?;
        let root = definition.checked_add(2)?;
        if root >= MAPPED_PAGES {
            return None;
        }
        let (owned, candidate, index) = if table == 0 {
            (FIRST_OWNED_SLOT, FIRST_CANDIDATE_SLOT, FIRST_INDEX_SLOT)
        } else {
            let owned = LATER_OWNED_BASE.checked_add(table.checked_mul(TABLE_PAGES)?)?;
            (owned, owned.checked_add(1)?, owned.checked_add(2)?)
        };
        u8::try_from(index).ok()?;
        let catalog = SYSTEM_OBJECTS.checked_add(table)?;
        u8::try_from(catalog).ok()?;
        Some(Self {
            definition,
            data,
            root,
            owned,
            candidate,
            index,
            catalog,
        })
    }
}

#[derive(Clone, Copy)]
pub(super) struct Geometry {
    pub tables: usize,
    pub after_pages: usize,
    pub catalog_rows: usize,
    pub map_rows: usize,
    pub added: Role,
}

impl Geometry {
    /// Preflight unsupported lengths without reading or allocating a snapshot.
    /// Every accepted role still needs complete source qualification afterwards.
    pub fn capture(bytes: usize) -> Option<Self> {
        if !bytes.is_multiple_of(PAGE_BYTES) {
            return None;
        }
        let before_pages = bytes / PAGE_BYTES;
        let user_pages = before_pages.checked_sub(SYSTEM_PAGES)?;
        if user_pages == 0 || !user_pages.is_multiple_of(TABLE_PAGES) {
            return None;
        }
        let tables = user_pages / TABLE_PAGES;
        let after_pages = before_pages.checked_add(TABLE_PAGES)?;
        if after_pages > MAPPED_PAGES {
            return None;
        }
        let added = Role::at(tables)?;
        let catalog_rows = SYSTEM_OBJECTS.checked_add(tables)?;
        let map_rows = RESERVED_MAPS.checked_add(tables.checked_mul(TABLE_PAGES)?)?;
        if added.definition != before_pages || added.owned != map_rows {
            return None;
        }
        Some(Self {
            tables,
            after_pages,
            catalog_rows,
            map_rows,
            added,
        })
    }

    pub fn table(self, index: usize) -> Option<Role> {
        if index >= self.tables {
            return None;
        }
        Role::at(index)
    }
}
