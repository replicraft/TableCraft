//! Read-only represented ownership and candidate maps shared by real LVAL consumers.
use crate::{
    EditError, InspectError, WriteBlocker,
    format::{
        allocation::{self, MapRole, RepresentedMap},
        reader::Reader,
    },
};
use std::io::Cursor;

pub(in crate::table::mutation) struct Scope {
    #[cfg(any(feature = "ace16-write-preview", all(feature = "journal", feature = "ace16-create")))]
    pub owned: RepresentedMap,
    #[cfg(any(feature = "ace16-write-preview", all(feature = "journal", feature = "ace16-create")))]
    pub hint: RepresentedMap,
    pub pages: Vec<u32>,
    #[cfg(feature = "ace16-write-preview")]
    pub hint_reference: u32,
}

impl Scope {
    pub(in crate::table::mutation) fn capture(
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        owned_reference: u32,
        hint_reference: u32,
        role: MapRole,
        global: &RepresentedMap,
    ) -> Result<Self, EditError> {
        if owned_reference == 0 || hint_reference == 0 || owned_reference == hint_reference {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let owned = RepresentedMap::read(reader, owned_reference, MapRole::Owned)?;
        let hint = RepresentedMap::read(reader, hint_reference, role)?;
        owned.require_allocated_storage(reader, global)?;
        hint.require_allocated_storage(reader, global)?;
        let pages = allocation::pages(reader, owned_reference)?;
        let candidates = allocation::pages(reader, hint_reference)?;
        let levels = pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
        for candidate in candidates {
            reader.tick()?;
            reader.work(levels)?;
            if pages.binary_search(&candidate).is_err() {
                return Err(InspectError::Malformed.into());
            }
        }
        for &number in &pages {
            global.require_allocated(reader, number)?;
        }

        Ok(Self {
            #[cfg(any(feature = "ace16-write-preview", all(feature = "journal", feature = "ace16-create")))]
            owned,
            #[cfg(any(feature = "ace16-write-preview", all(feature = "journal", feature = "ace16-create")))]
            hint,
            pages,
            #[cfg(feature = "ace16-write-preview")]
            hint_reference,
        })
    }
}
