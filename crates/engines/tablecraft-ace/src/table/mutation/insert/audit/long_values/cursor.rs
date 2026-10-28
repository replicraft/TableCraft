//! SOURCE-only LVAL candidate replay and exact append bytes on the shared EOF.
use super::super::{ImageProof, reserve};
use super::Declaration;
use super::maps::{Allocation, Authority, Bitmap, Maps};
use crate::{
    EditError, InspectError, LongValueMapReferences, TableSchema,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u16_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{MAX_SLOTS, OFFSET_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, reference, slot_offset},
    },
    table::mutation::{
        insert::long_values::Plan,
        long_value::{maps::Scope, ownership::Inventory},
        variable_update::map_audit::expect,
    },
};
use std::ops::Range;

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const LVAL_OWNER: &[u8; size_of::<u32>()] = b"LVAL";
const EMPTY_FREE: usize = PAGE_BYTES - ROW_COUNT_OFFSET - size_of::<u16>();

struct State {
    number: u32,
    count: usize,
    end: usize,
    hinted: bool,
}

struct Column {
    position: usize,
    number: usize,
    references: LongValueMapReferences,
    maps: Maps,
    states: Vec<State>,
}

struct Touched {
    number: u32,
    column: usize,
    references: LongValueMapReferences,
}

pub(super) struct Cursor<'a> {
    schema: &'a TableSchema,
    inventory: Inventory,
    bitmaps: Vec<Bitmap>,
    original_eof: u32,
    global: RepresentedMap,
    columns: Vec<Column>,
    touched: Vec<Touched>,
}

pub(super) struct Location {
    pub reference: u32,
    pub range: Range<usize>,
}

impl<'a> Cursor<'a> {
    pub(super) fn capture(
        schema: &'a TableSchema,
        selected: &[Declaration],
        proof: &ImageProof<'_>,
        inventory: Inventory,
        reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(reader, &global)?;
        let mut columns = Vec::new();
        reserve(&mut columns, selected.len(), reader)?;
        for declaration in selected {
            let position = declaration.position;
            let references = declaration.references;
            reader.tick()?;
            let field = schema
                .columns()
                .get(position)
                .ok_or(EditError::Preservation)?;
            let mut states = Vec::new();
            let maps = if let Some(number) = declaration.initialized {
                Maps::initialized(number, references, proof)?
            } else {
                let scope = Scope::capture(
                    reader,
                    references.owned(),
                    references.candidate(),
                    MapRole::LongValueCandidate,
                    &global,
                )?;
                inventory.require_exclusive(
                    schema,
                    field.number(),
                    &scope.pages,
                    reader.budget,
                    reader.cancelled,
                )?;
                reserve(&mut states, scope.pages.len(), reader)?;
                for &number in &scope.pages {
                    reader.tick()?;
                    let page = reader.page(number)?;
                    let (count, end) = geometry(&page)?;
                    let hinted = match scope.hint.cell(reader, number) {
                        Ok(cell) => cell.before & cell.mask != 0,
                        Err(InspectError::Unsupported) => false,
                        Err(error) => return Err(error.into()),
                    };
                    states.push(State {
                        number,
                        count,
                        end,
                        hinted,
                    });
                }
                Maps::capture(references, reader)?
            };
            columns.push(Column {
                position,
                number: field.number(),
                references,
                maps,
                states,
            });
        }
        Ok(Self {
            schema,
            inventory,
            bitmaps: Vec::new(),
            original_eof: reader.header.page_count(),
            global,
            columns,
            touched: Vec::new(),
        })
    }

    fn touch(
        &mut self,
        column: usize,
        number: u32,
        reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.work(self.touched.len())?;
        if let Some(prior) = self.touched.iter().find(|page| page.number == number) {
            if prior.column != self.columns[column].number {
                return Err(EditError::Preservation);
            }
            return Ok(());
        }
        reserve(&mut self.touched, 1, reader)?;
        self.touched.push(Touched {
            number,
            column: self.columns[column].number,
            references: self.columns[column].references,
        });
        Ok(())
    }

    pub(super) fn append(
        &mut self,
        selected: usize,
        body: &[u8],
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    ) -> Result<Location, EditError> {
        reader.tick()?;
        reader
            .budget
            .charge(tablecraft_core::limits::Resource::Rows, 1)
            .map_err(InspectError::from)?;
        if body.is_empty()
            || body
                .len()
                .checked_add(SLOT_BYTES)
                .is_none_or(|bytes| bytes > EMPTY_FREE)
        {
            return Err(EditError::Preservation);
        }
        reader.work(self.columns.len())?;
        let column = self
            .columns
            .iter()
            .position(|column| column.position == selected)
            .ok_or(EditError::Preservation)?;
        let destination = loop {
            reader.work(self.columns[column].states.len())?;
            let highest = self.columns[column]
                .states
                .iter()
                .rposition(|state| state.hinted);
            let Some(position) = highest else {
                break None;
            };
            let state = &self.columns[column].states[position];
            let number = state.number;
            let free = state
                .end
                .checked_sub(slot_offset(state.count))
                .ok_or(EditError::Preservation)?;
            let fits = state.count < MAX_SLOTS && body.len() + SLOT_BYTES <= free;
            self.touch(column, number, reader)?;
            if fits {
                break Some(position);
            }
            self.columns[column].states[position].hinted = false;
            let mut remaining = Vec::new();
            reserve(&mut remaining, self.columns[column].states.len(), reader)?;
            for state in &self.columns[column].states {
                reader.tick()?;
                if state.hinted {
                    remaining.push(state.number);
                }
            }
            let selected = &mut self.columns[column];
            selected.maps.clear_hint(
                number,
                &remaining,
                Allocation {
                    authority: Authority {
                        schema: self.schema,
                        inventory: &self.inventory,
                        column: selected.number,
                    },
                    global: &self.global,
                    members: &[],
                },
                proof,
                reader,
            )?;
        };
        let position = if let Some(position) = destination {
            position
        } else {
            let mut members = Vec::new();
            reserve(&mut members, self.columns[column].states.len(), reader)?;
            for state in &self.columns[column].states {
                reader.tick()?;
                members.push(state.number);
            }
            let selected = &mut self.columns[column];
            let number = selected.maps.allocate(
                Allocation {
                    authority: Authority {
                        schema: self.schema,
                        inventory: &self.inventory,
                        column: selected.number,
                    },
                    global: &self.global,
                    members: &members,
                },
                &mut self.bitmaps,
                proof,
                reader,
            )?;
            let base = number as usize * PAGE_BYTES;
            proof.bytes[base..base + DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
            proof.bytes[base + OWNER_OFFSET..base + OWNER_OFFSET + LVAL_OWNER.len()]
                .copy_from_slice(LVAL_OWNER);
            proof.bytes[base + FREE_OFFSET..base + FREE_OFFSET + SLOT_BYTES]
                .copy_from_slice(&(EMPTY_FREE as u16).to_le_bytes());
            reserve(&mut self.columns[column].states, 1, reader)?;
            let position = self.columns[column].states.len();
            self.columns[column].states.push(State {
                number,
                count: 0,
                end: PAGE_BYTES,
                hinted: true,
            });
            self.touch(column, number, reader)?;
            position
        };
        let state = &mut self.columns[column].states[position];
        let begin = state
            .end
            .checked_sub(body.len())
            .filter(|begin| *begin >= slot_offset(state.count + 1))
            .ok_or(EditError::Preservation)?;
        let word = u16::try_from(begin).map_err(|_| InspectError::StructureLimit)?;
        if word & !OFFSET_MASK != 0 {
            return Err(EditError::Preservation);
        }
        let base = state.number as usize * PAGE_BYTES;
        reader.work(body.len() + SLOT_BYTES * 3)?;
        proof.bytes[base + begin..base + state.end].copy_from_slice(body);
        let slot = state.count;
        proof.bytes[base + slot_offset(slot)..base + slot_offset(slot) + SLOT_BYTES]
            .copy_from_slice(&word.to_le_bytes());
        state.count += 1;
        proof.bytes[base + ROW_COUNT_OFFSET..base + ROW_COUNT_OFFSET + SLOT_BYTES]
            .copy_from_slice(&(state.count as u16).to_le_bytes());
        proof.bytes[base + FREE_OFFSET..base + FREE_OFFSET + SLOT_BYTES]
            .copy_from_slice(&((begin - slot_offset(state.count)) as u16).to_le_bytes());
        let location = Location {
            reference: reference(
                state.number,
                u8::try_from(slot).map_err(|_| InspectError::StructureLimit)?,
            ),
            range: base + begin..base + state.end,
        };
        state.end = begin;
        Ok(location)
    }

    pub(super) fn require_purposes(
        &self,
        staged: &crate::table::mutation::insert::Staged,
        reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let mut actual = staged.data.append.long_values();
        for page in &self.touched {
            reader.tick()?;
            if page.number >= self.original_eof {
                let reservation = actual.next().ok_or(EditError::Preservation)?;
                if reservation.number != page.number
                    || reservation.column != page.column
                    || reservation.owned != page.references.owned()
                    || reservation.candidate != page.references.candidate()
                {
                    return Err(EditError::Preservation);
                }
                staged.data.append.require_long_value(
                    page.number,
                    page.column,
                    page.references.owned(),
                    page.references.candidate(),
                    reader.budget,
                    reader.cancelled,
                )?;
            }
        }
        if actual.next().is_some() {
            return Err(EditError::Preservation);
        }
        let mut metadata = staged.data.append.long_value_owned_bitmaps();
        for expected in &self.bitmaps {
            reader.tick()?;
            let actual = metadata.next().ok_or(EditError::Preservation)?;
            if actual.number != expected.number
                || actual.column != expected.column
                || actual.owned != expected.owned
                || actual.block != expected.block
            {
                return Err(EditError::Preservation);
            }
            staged.data.append.require_long_value_owned_bitmap(
                expected.number,
                expected.column,
                expected.owned,
                expected.block,
                reader.budget,
                reader.cancelled,
            )?;
        }
        if metadata.next().is_some() {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(super) fn compare(
        &self,
        actual: &Plan,
        proof: &ImageProof<'_>,
        reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if self.touched.len() != actual.pages.len() || self.bitmaps.len() != actual.metadata.len() {
            return Err(EditError::Preservation);
        }
        for (expected, actual) in self.touched.iter().zip(&actual.pages) {
            reader.work(PAGE_BYTES)?;
            let base = expected.number as usize * PAGE_BYTES;
            let before = proof.original.get(base..base + PAGE_BYTES).unwrap_or(&[]);
            if actual.number != expected.number
                || actual.column != expected.column
                || actual.owned != expected.references.owned()
                || actual.candidate != expected.references.candidate()
                || actual.before != before
                || actual.after != proof.bytes[base..base + PAGE_BYTES]
            {
                return Err(EditError::Preservation);
            }
        }
        for (expected, actual) in self.bitmaps.iter().zip(&actual.metadata) {
            reader.work(PAGE_BYTES)?;
            let base = expected.number as usize * PAGE_BYTES;
            if actual.number != expected.number
                || actual.column != expected.column
                || actual.owned != expected.owned
                || actual.block != expected.block
                || actual.bytes != proof.bytes[base..base + PAGE_BYTES]
            {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }
}

fn geometry(page: &[u8]) -> Result<(usize, usize), EditError> {
    if !page.starts_with(&DATA_SIGNATURE)
        || page.get(OWNER_OFFSET..OWNER_OFFSET + LVAL_OWNER.len()) != Some(LVAL_OWNER.as_slice())
    {
        return Err(EditError::Preservation);
    }
    let count = row_count(page)?;
    let end = if count == 0 {
        PAGE_BYTES
    } else {
        row_range(page, count - 1)?.1
    };
    let free = end
        .checked_sub(slot_offset(count))
        .ok_or(EditError::Preservation)?;
    if usize::from(u16_at(page, FREE_OFFSET)?) != free {
        return Err(EditError::Preservation);
    }
    Ok((count, end))
}

fn cell(
    map: &RepresentedMap,
    eof: u32,
    number: u32,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
) -> Result<crate::format::allocation::MapCell, EditError> {
    Ok(if number < eof {
        map.cell(reader, number)?
    } else {
        map.future_cell(reader, number)?
    })
}

pub(super) fn require_bit(
    proof: &ImageProof<'_>,
    map: &RepresentedMap,
    eof: u32,
    number: u32,
    wanted: bool,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let cell = cell(map, eof, number, reader)?;
    let current = *proof
        .bytes
        .get(cell.offset)
        .ok_or(EditError::Preservation)?;
    if (current & cell.mask != 0) != wanted {
        return Err(EditError::Preservation);
    }
    Ok(())
}

pub(super) fn change(
    proof: &mut ImageProof<'_>,
    map: &RepresentedMap,
    eof: u32,
    number: u32,
    set: bool,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let cell = cell(map, eof, number, reader)?;
    let offset = cell.offset;
    expect(&mut proof.map_edits, cell, set, reader)?;
    let expected = proof
        .map_edits
        .iter()
        .find(|byte| byte.offset == offset)
        .ok_or(EditError::Preservation)?;
    *proof.bytes.get_mut(offset).ok_or(EditError::Preservation)? = expected.after;
    Ok(())
}
