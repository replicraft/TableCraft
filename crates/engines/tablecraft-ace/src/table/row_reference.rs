use crate::{
    InspectError,
    format::{page::PAGE_NUMBER_MASK, row},
};

/// A validated logical DATA root within one stable source and table definition.
/// Equality/order are meaningful only with the returned schema and the caller's
/// unchanged source binding. This is not a durable row ID, physical forwarding
/// target, mutation capability or authority to reopen another source.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeRowReference(u32);

impl NativeRowReference {
    pub(in crate::table) const fn from_packed(reference: u32) -> Self {
        Self(reference)
    }

    pub(in crate::table) fn from_parts(page: u32, slot: u8) -> Result<Self, InspectError> {
        if page > PAGE_NUMBER_MASK {
            return Err(InspectError::Malformed);
        }
        Ok(Self(row::reference(page, slot)))
    }

    /// Original logical DATA page, before following any forwarding pointer.
    pub const fn page_number(self) -> u32 {
        row::reference_page(self.0)
    }

    /// Original live root slot on that DATA page.
    pub const fn slot_number(self) -> u8 {
        row::reference_slot(self.0)
    }
}
