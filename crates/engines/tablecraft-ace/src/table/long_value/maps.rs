//! Qualified trailing column pairs and role-aware LVAL storage inventories.
use super::LVAL_SIGNATURE;
use super::model::LongValueMapReferences;
use crate::{
    InspectError, NativeReadSource, TableSchema,
    format::{
        allocation::{self, MapCell, MapRole, RepresentedMap},
        bytes::u16_at,
        definition,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::slot_offset,
    },
    limits::MAX_COLUMNS,
    table::indexes,
};
use zerocopy::{
    FromBytes, Immutable, KnownLayout, Unaligned,
    little_endian::{U16, U32},
};

#[cfg(feature = "ace16-write-preview")]
use crate::format::bytes::u32_at;

const TERMINATOR: u16 = u16::MAX;
const INLINE_MAP: u8 = 0;
const INDIRECT_MAP: u8 = 1;
const FREE_OFFSET: usize = DATA_SIGNATURE.len();

#[cfg(feature = "ace16-write-preview")]
const DEFINITION_FREE_OFFSET: usize = 2;
#[cfg(feature = "ace16-write-preview")]
const DEFINITION_NEXT_OFFSET: usize = 4;
#[cfg(feature = "ace16-write-preview")]
const DEFINITION_LENGTH_OFFSET: usize = 8;
#[cfg(feature = "ace16-write-preview")]
const DEFINITION_HEADER_BYTES: usize = 8;
#[cfg(feature = "ace16-write-preview")]
const TRAILER_BYTES: usize = size_of::<u16>();

/// Complete original single-page tail, separate from any zero-pair patch.
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) struct DefinitionTail {
    pub definition: u32,
    pub terminal: usize,
    pub old_length: usize,
    pub old_free: u16,
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) fn definition_tail<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
) -> Result<DefinitionTail, InspectError> {
    let definitions = indexes::definitions_with_tail(reader, schema)?;
    let bytes = definition::read(reader, schema.definition)?;
    if bytes.len() > PAGE_BYTES || u32_at(&bytes, DEFINITION_NEXT_OFFSET)? != 0 {
        return Err(InspectError::Unsupported);
    }
    let old_length = u32_at(&bytes, DEFINITION_LENGTH_OFFSET)? as usize;
    let old_free = u16_at(&bytes, DEFINITION_FREE_OFFSET)?;
    if usize::from(old_free) != PAGE_BYTES.saturating_sub(old_length + DEFINITION_HEADER_BYTES) {
        return Err(InspectError::Unsupported);
    }
    let mut position = definitions.tail;
    for _ in 0..=MAX_COLUMNS {
        reader.tick()?;
        if u16_at(&bytes, position)? == TERMINATOR {
            if position.checked_add(TRAILER_BYTES) != Some(old_length) {
                return Err(InspectError::Unsupported);
            }
            let terminal = (schema.definition as usize)
                .checked_mul(PAGE_BYTES)
                .and_then(|base| base.checked_add(position))
                .ok_or(InspectError::StructureLimit)?;
            return Ok(DefinitionTail {
                definition: schema.definition,
                terminal,
                old_length,
                old_free,
            });
        }
        position = position
            .checked_add(size_of::<Pair>())
            .ok_or(InspectError::StructureLimit)?;
        if position > bytes.len() {
            return Err(InspectError::Truncated);
        }
    }
    Err(InspectError::StructureLimit)
}

#[derive(FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C)]
struct Pair {
    column: U16,
    owned: U32,
    candidate: U32,
}

pub(in crate::table) struct Declaration {
    pub number: usize,
    pub references: LongValueMapReferences,
}

pub(in crate::table) struct Declarations {
    pub entries: Vec<Declaration>,
    pub complete: bool,
}

pub(in crate::table) fn declarations<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
) -> Result<Declarations, InspectError> {
    let definitions = match indexes::definitions_with_tail(reader, schema) {
        Ok(value) => value,
        Err(InspectError::Unsupported) => {
            return Ok(Declarations {
                entries: Vec::new(),
                complete: false,
            });
        }
        Err(error) => return Err(error),
    };
    let bytes = definition::read(reader, schema.definition)?;
    parse(&bytes, definitions.tail, reader, schema, &[])
}

/// Explicit INSERT initialization parses zero pairs without treating unknown
/// trailing bytes or missing declarations as an empty ownership claim.
pub(in crate::table) fn declarations_for_initialization<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    columns: &[usize],
) -> Result<Declarations, InspectError> {
    let definitions = indexes::definitions_with_tail(reader, schema)?;
    let bytes = definition::read(reader, schema.definition)?;
    let declarations = parse(&bytes, definitions.tail, reader, schema, columns)?;
    if bytes.len() > PAGE_BYTES {
        reader.work(declarations.entries.len())?;
        if declarations
            .entries
            .iter()
            .any(|entry| entry.references.owned() == 0 && entry.references.candidate() == 0)
        {
            return Err(InspectError::Unsupported);
        }
    }
    Ok(declarations)
}

/// Exact original physical pointer span for one explicitly declared zero pair.
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) fn zero_pair_offset<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    column: usize,
) -> Result<usize, InspectError> {
    let definitions = indexes::definitions_with_tail(reader, schema)?;
    let bytes = definition::read(reader, schema.definition)?;
    if bytes.len() > PAGE_BYTES {
        return Err(InspectError::Unsupported);
    }
    let mut position = definitions.tail;
    loop {
        reader.tick()?;
        let number = u16_at(&bytes, position)?;
        if number == TERMINATOR {
            return Err(InspectError::Unsupported);
        }
        let pair = bytes
            .get(position..position + size_of::<Pair>())
            .ok_or(InspectError::Truncated)?;
        if usize::from(number) == column {
            if pair[2..].iter().any(|byte| *byte != 0) {
                return Err(InspectError::Unsupported);
            }
            return (schema.definition as usize)
                .checked_mul(PAGE_BYTES)
                .and_then(|base| base.checked_add(position + size_of::<u16>()))
                .ok_or(InspectError::StructureLimit);
        }
        position = position
            .checked_add(size_of::<Pair>())
            .ok_or(InspectError::StructureLimit)?;
    }
}

fn parse<R: NativeReadSource>(
    bytes: &[u8],
    mut position: usize,
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    initializing: &[usize],
) -> Result<Declarations, InspectError> {
    let mut entries: Vec<Declaration> = Vec::new();
    for _ in 0..=MAX_COLUMNS {
        reader.tick()?;
        if bytes.len().saturating_sub(position) < size_of::<u16>() {
            return Ok(Declarations {
                entries,
                complete: false,
            });
        }

        let column = u16_at(bytes, position)?;
        if column == TERMINATOR {
            return Ok(Declarations {
                entries,
                complete: true,
            });
        }

        let rest = bytes.get(position..).ok_or(InspectError::Truncated)?;
        let (pair, _) = Pair::ref_from_prefix(rest).map_err(|_| InspectError::Truncated)?;
        let number = usize::from(pair.column.get());
        reader.work(entries.len())?;
        if entries.iter().any(|entry| entry.number == number) {
            reader.work(schema.columns.len())?;
            let known = schema.columns.iter().any(|column| {
                column.number() == number
                    && matches!(
                        column.native_type(),
                        crate::format::data_type::LONG_TEXT | crate::format::data_type::OLE
                    )
            });
            if known {
                return Err(InspectError::Malformed);
            }
            return Ok(Declarations {
                entries,
                complete: false,
            });
        }
        reader
            .budget
            .charge_allocation(1, size_of::<Declaration>() as u64)?;
        entries
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        entries.push(Declaration {
            number,
            references: LongValueMapReferences {
                owned: pair.owned.get(),
                candidate: pair.candidate.get(),
            },
        });
        position = position
            .checked_add(size_of::<Pair>())
            .ok_or(InspectError::StructureLimit)?;
        // Zero pairs are absent/legacy leads, not permission to interpret an
        // arbitrary zero-filled definition suffix as a complete declaration list.
        let selected_zero =
            if pair.owned.get() == 0 && pair.candidate.get() == 0 && !initializing.is_empty() {
                reader.work(initializing.len())?;
                reader.work(schema.columns.len())?;
                initializing.contains(&number)
                    && schema.columns.iter().any(|field| {
                        field.number() == number
                            && matches!(
                                field.native_type(),
                                crate::format::data_type::LONG_TEXT | crate::format::data_type::OLE
                            )
                    })
            } else {
                false
            };
        if (pair.owned.get() == 0 || pair.candidate.get() == 0) && !selected_zero {
            return Ok(Declarations {
                entries,
                complete: false,
            });
        }
    }
    Err(InspectError::StructureLimit)
}

pub(super) enum MapInspection {
    Qualified(RepresentedMap, RepresentedMap),
    Indirect,
    Unknown,
}

pub(super) fn inspect<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    references: LongValueMapReferences,
) -> Result<MapInspection, InspectError> {
    if references.owned == references.candidate {
        return Ok(MapInspection::Unknown);
    }

    let owned = match RepresentedMap::read(reader, references.owned, MapRole::Owned) {
        Ok(value) => value,
        Err(InspectError::Unsupported) => return Ok(MapInspection::Unknown),
        Err(error) => return Err(error),
    };
    let candidate =
        match RepresentedMap::read(reader, references.candidate, MapRole::LongValueCandidate) {
            Ok(value) => value,
            Err(InspectError::Unsupported) => return Ok(MapInspection::Unknown),
            Err(error) => return Err(error),
        };
    match (owned.kind(), candidate.kind()) {
        (INLINE_MAP | INDIRECT_MAP, INLINE_MAP) => Ok(MapInspection::Qualified(owned, candidate)),
        (INLINE_MAP | INDIRECT_MAP, INDIRECT_MAP) => Ok(MapInspection::Indirect),
        _ => Ok(MapInspection::Unknown),
    }
}

pub(super) struct Storage {
    pub pages: Vec<u32>,
    pub rows: usize,
    pub candidate_pages: usize,
    pub reached: Vec<u32>,
}

pub(super) fn capture<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    references: LongValueMapReferences,
    owned: &RepresentedMap,
    candidate: &RepresentedMap,
    global: &RepresentedMap,
) -> Result<Storage, InspectError> {
    owned.require_allocated_storage(reader, global)?;
    candidate.require_allocated_storage(reader, global)?;
    let pages = allocation::pages(reader, references.owned)?;
    let candidates = allocation::pages(reader, references.candidate)?;
    let levels = pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
    for number in &candidates {
        reader.work(levels)?;
        if pages.binary_search(number).is_err() {
            return Err(InspectError::Malformed);
        }
    }

    let mut rows: usize = 0;
    for number in &pages {
        allocated(reader, global, *number)?;
        let page = reader.page(*number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || page.get(OWNER_OFFSET..OWNER_OFFSET + LVAL_SIGNATURE.len()) != Some(LVAL_SIGNATURE)
        {
            return Err(InspectError::Malformed);
        }

        let count = row_count(&page)?;
        let end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(&page, count - 1)?.1
        };
        if usize::from(u16_at(&page, FREE_OFFSET)?) != end - slot_offset(count) {
            return Err(InspectError::Malformed);
        }
        for slot in 0..count {
            reader.tick()?;
            row_range(&page, slot)?;
        }
        rows = rows
            .checked_add(count)
            .ok_or(InspectError::StructureLimit)?;
    }
    Ok(Storage {
        pages,
        rows,
        candidate_pages: candidates.len(),
        reached: Vec::new(),
    })
}

pub(super) fn allocated<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    global: &RepresentedMap,
    page: u32,
) -> Result<(), InspectError> {
    let cell: MapCell = global.cell(reader, page)?;
    if cell.offset >= reader.header.page_count() as usize * PAGE_BYTES
        || cell.before & cell.mask != 0
    {
        return Err(InspectError::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Pair;
    use zerocopy::FromBytes;

    #[test]
    fn unaligned_ten_byte_column_pair_matches_both_original_ace16_goldens() {
        for bytes in [
            [99, 1, 0, 3, 242, 0, 0, 4, 242, 0, 0],
            [99, 2, 0, 5, 3, 1, 0, 6, 3, 1, 0],
        ] {
            let (pair, tail) = Pair::ref_from_prefix(&bytes[1..]).unwrap();
            assert!(tail.is_empty());
            assert_eq!((size_of::<Pair>(), align_of::<Pair>()), (10, 1));
            assert_eq!(pair.column.get(), if bytes[1] == 1 { 1 } else { 2 });
            assert_eq!(
                pair.owned.get(),
                if bytes[1] == 1 {
                    0x0000_f203
                } else {
                    0x0001_0305
                }
            );
            assert_eq!(
                pair.candidate.get(),
                if bytes[1] == 1 {
                    0x0000_f204
                } else {
                    0x0001_0306
                }
            );
            assert!(Pair::ref_from_prefix(&bytes[1..10]).is_err());
        }
    }
}
