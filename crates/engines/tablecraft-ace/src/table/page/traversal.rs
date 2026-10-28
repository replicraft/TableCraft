//! Shared resumable physical root traversal; records are decoded by the caller.
use super::*;
use crate::format::page::PAGE_BYTES;

pub(super) struct Root {
    number: u32,
    slot: u8,
}
impl Root {
    pub(super) fn row_reference(&self) -> Result<crate::table::NativeRowReference, InspectError> {
        crate::table::NativeRowReference::from_parts(self.number, self.slot)
    }
}
struct CurrentPage {
    number: u32,
    bytes: [u8; PAGE_BYTES],
    slots: usize,
    next_slot: usize,
}
pub(super) struct Traversal {
    pages: allocation::PageCursor,
    current: Option<CurrentPage>,
    visited: u64,
    ended: bool,
}
impl Traversal {
    pub(super) fn new<R: crate::NativeReadSource>(
        reader: &mut Reader<'_, R>,
        schema: &TableSchema,
    ) -> Result<Self, InspectError> {
        Ok(Self {
            pages: allocation::PageCursor::new(reader, schema.layout.usage)?,
            current: None,
            visited: 0,
            ended: false,
        })
    }
    pub(super) fn visited(&self) -> u64 {
        self.visited
    }
    pub(super) fn ended(&self) -> bool {
        self.ended
    }
    pub(super) fn next<R: crate::NativeReadSource>(
        &mut self,
        reader: &mut Reader<'_, R>,
        schema: &TableSchema,
    ) -> Result<Option<Root>, InspectError> {
        reader.tick()?;
        if self.ended {
            return Ok(None);
        }
        loop {
            if let Some(page) = &mut self.current {
                while page.next_slot < page.slots {
                    reader.tick()?;
                    let slot = page.next_slot;
                    page.next_slot += 1;
                    if u16_at(&page.bytes, slot_offset(slot))? & DELETED_MASK != 0 {
                        continue;
                    }
                    self.visited += 1;
                    if self.visited > u64::from(schema.declared_rows()) {
                        return Err(InspectError::Malformed);
                    }
                    return Ok(Some(Root {
                        number: page.number,
                        slot: slot.try_into().map_err(|_| InspectError::Malformed)?,
                    }));
                }
            }
            self.current = None;
            let Some(number) = self.pages.next(reader)? else {
                if self.visited != u64::from(schema.declared_rows()) {
                    return Err(InspectError::Malformed);
                }
                self.ended = true;
                return Ok(None);
            };
            let bytes = reader.page(number)?;
            if bytes[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || u32_at(&bytes, OWNER_OFFSET)? != schema.definition
            {
                return Err(InspectError::Malformed);
            }
            let slots = row_count(&bytes)?;
            self.current = Some(CurrentPage {
                number,
                bytes,
                slots,
                next_slot: 0,
            });
        }
    }
    pub(super) fn skip(&self, root: &Root) -> Result<(), InspectError> {
        row_range(self.bytes(root)?, usize::from(root.slot))?;
        Ok(())
    }
    pub(super) fn values<R: crate::NativeReadSource>(
        &self,
        root: &Root,
        reader: &mut Reader<'_, R>,
        schema: &TableSchema,
        columns: &[Column],
        selected: Option<&[usize]>,
    ) -> Result<Vec<Value>, InspectError> {
        reader.budget.charge(Resource::Rows, 1)?;
        let row =
            reader.row_from_page(root.number, root.slot, schema.definition, self.bytes(root)?)?;
        let values = record::decode_selected(&row, schema, selected, reader)?;
        validate_row(columns, &values)?;
        Ok(values)
    }
    fn bytes(&self, root: &Root) -> Result<&[u8; PAGE_BYTES], InspectError> {
        self.current
            .as_ref()
            .filter(|page| page.number == root.number)
            .map(|page| &page.bytes)
            .ok_or(InspectError::Malformed)
    }
}
