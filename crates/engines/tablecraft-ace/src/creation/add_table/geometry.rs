//! Bounded structural roles; file length is framing, never ownership evidence.
use crate::format::page::PAGE_BYTES;

pub(super) const SYSTEM_PAGES: usize = 7;
pub(super) const SYSTEM_OBJECTS: usize = 7;
const LEGACY_TABLE_PAGES: usize = 3;
const RESERVED_MAPS: usize = 20;
const MAP_BITMAP_BYTES: usize = 8;
pub(super) const MAPPED_PAGES: usize = MAP_BITMAP_BYTES * u8::BITS as usize;
pub(super) const MAX_TABLES: usize = (MAPPED_PAGES - SYSTEM_PAGES) / 2;

#[derive(Clone, Copy)]
pub(in crate::creation) struct PrimaryRole {
    pub root: usize,
    pub map: usize,
}

#[derive(Clone, Copy)]
pub(in crate::creation) struct Role {
    pub definition: usize,
    pub data: usize,
    pub primary: Option<PrimaryRole>,
    pub owned: usize,
    pub candidate: usize,
    pub catalog: usize,
}

impl Role {
    pub fn at(table: usize, definition: usize, keyed: bool) -> Option<Self> {
        let data = definition.checked_add(1)?;
        let root = keyed.then(|| definition.checked_add(2)).flatten();
        if data >= MAPPED_PAGES || root.is_some_and(|root| root >= MAPPED_PAGES) {
            return None;
        }
        let user_pages = definition.checked_sub(SYSTEM_PAGES)?;
        let (owned, candidate) = if table == 0 {
            (3, 7)
        } else {
            let owned = RESERVED_MAPS.checked_add(user_pages)?;
            (owned, owned.checked_add(1)?)
        };
        let primary = root.map(|root| PrimaryRole {
            root,
            map: RESERVED_MAPS + user_pages + 2,
        });
        u8::try_from(primary.map_or(candidate, |primary| primary.map)).ok()?;
        let catalog = SYSTEM_OBJECTS.checked_add(table)?;
        u8::try_from(catalog).ok()?;
        Some(Self {
            definition,
            data,
            primary,
            owned,
            candidate,
            catalog,
        })
    }

    pub fn pages(self) -> usize {
        2 + usize::from(self.primary.is_some())
    }

    pub fn members(self) -> impl Iterator<Item = usize> {
        [
            Some(self.data),
            Some(self.data),
            self.primary.map(|primary| primary.root),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Clone, Copy)]
pub(in crate::creation) struct Geometry {
    pub tables: usize,
    pub before_pages: usize,
    pub after_pages: usize,
    pub catalog_rows: usize,
    pub map_rows: usize,
    pub added: Role,
    pub roles: [Option<Role>; MAX_TABLES],
}

impl Geometry {
    /// Legacy three-page framing; semantic qualification remains unchanged.
    pub fn capture(bytes: usize) -> Option<Self> {
        if !bytes.is_multiple_of(PAGE_BYTES) {
            return None;
        }
        let before_pages = bytes / PAGE_BYTES;
        let user_pages = before_pages.checked_sub(SYSTEM_PAGES)?;
        if user_pages == 0 || !user_pages.is_multiple_of(LEGACY_TABLE_PAGES) {
            return None;
        }
        let tables = user_pages / LEGACY_TABLE_PAGES;
        let mut roles = [None; MAX_TABLES];
        if tables > roles.len() {
            return None;
        }
        for (index, slot) in roles.iter_mut().take(tables).enumerate() {
            *slot = Some(Role::at(
                index,
                SYSTEM_PAGES + index * LEGACY_TABLE_PAGES,
                true,
            )?);
        }
        Self::from_roles(roles, tables, before_pages, true)
    }

    pub fn from_roles(
        roles: [Option<Role>; MAX_TABLES],
        tables: usize,
        before_pages: usize,
        keyed: bool,
    ) -> Option<Self> {
        if tables == 0 || tables > roles.len() {
            return None;
        }
        let added = Role::at(tables, before_pages, keyed)?;
        let after_pages = before_pages.checked_add(added.pages())?;
        if after_pages > MAPPED_PAGES {
            return None;
        }
        Some(Self {
            tables,
            before_pages,
            after_pages,
            catalog_rows: SYSTEM_OBJECTS + tables,
            map_rows: RESERVED_MAPS + before_pages.checked_sub(SYSTEM_PAGES)?,
            added,
            roles,
        })
    }

    pub fn table(&self, index: usize) -> Option<Role> {
        self.roles
            .get(index)
            .copied()
            .flatten()
            .filter(|_| index < self.tables)
    }
}
