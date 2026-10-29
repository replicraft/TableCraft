//! Independently bind every physical owner and its complete reachable storage.
use super::super::super::fixed_batch::Input;
use super::push;
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeIndex, TableCheck, TableSchema, WriteBlocker,
    format::{allocation, page::PAGE_BYTES, reader::Reader},
    limits::MAX_INDEX_DEPTH,
    table::{
        indexes::{
            header::IndexHeader,
            mutation::IndexKeyUpdate,
            page::{ENTRY_START, LEAF, NODE},
        },
        long_value::maps::Declarations,
        mutation::map_authority::Owner,
    },
};
use std::io::Cursor;

const CHILD_BYTES: usize = size_of::<u32>();
const MASK_START: usize = size_of::<IndexHeader>();

pub(super) struct Physical<'a> {
    pub checked: &'a TableCheck,
    pub index: &'a NativeIndex,
    pub updates: &'a [IndexKeyUpdate],
}

pub(super) struct Source<'a> {
    pub physical: Vec<Physical<'a>>,
    pub owners: Vec<Owner>,
    pub index_pages: Vec<u32>,
}

impl Source<'_> {
    pub(super) fn require_catalog_claims(
        &self,
        schema: &TableSchema,
        indexes: &[NativeIndex],
        declarations: &Declarations,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        self.exclude(schema.layout.usage, reader)?;
        for column in &declarations.entries {
            reader.tick()?;
            self.exclude(column.references.owned(), reader)?;
        }
        for index in indexes {
            reader.tick()?;
            reader.work(self.physical.len())?;
            if let Some(selected) = self.physical.iter().find(|selected| {
                selected.checked.schema().definition_page() == schema.definition_page()
                    && selected.index.data_number() == index.data_number()
            }) {
                if selected.index.root_page() != index.root_page()
                    || selected.index.usage_map() != index.usage_map()
                {
                    return Err(EditError::Preservation);
                }
                // This exact owner's complete membership was derived above.
                // Every other owned role must remain disjoint from its pages.
                continue;
            }
            self.exclude(index.usage_map(), reader)?;
        }
        Ok(())
    }

    fn exclude(
        &self,
        reference: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        for page in allocation::pages(reader, reference)? {
            reader.tick()?;
            reader.work(self.index_pages.len())?;
            if self.index_pages.contains(&page) {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }
}

pub(super) fn capture<'a>(
    inputs: &[Input<'a>],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Source<'a>, EditError> {
    let mut tables = Vec::new();
    for input in inputs {
        reader.tick()?;
        reader.work(tables.len())?;
        if tables.iter().any(|prior: &&Input<'_>| {
            prior.checked.schema().definition_page() == input.checked.schema().definition_page()
        }) {
            return Err(EditError::Preservation);
        }
        push(&mut tables, input, reader)?;
    }
    let levels = tables.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(tables.len().saturating_mul(levels))?;
    tables.sort_unstable_by_key(|input| input.checked.schema().definition_page());

    let mut physical = Vec::new();
    let mut owners = Vec::new();
    let mut source_pages = Vec::new();
    push(&mut owners, Owner::GlobalFree, reader)?;
    for input in tables {
        let checked = input.checked;
        let schema = checked.schema();
        let definition = schema.definition_page();
        super::super::allocated_source(reader, schema, checked.indexes())?;
        push(&mut owners, Owner::OrdinaryOwned { definition }, reader)?;
        push(&mut owners, Owner::OrdinaryCandidate { definition }, reader)?;
        for number in 0..schema.layout.physical_indexes {
            reader.tick()?;
            reader.work(checked.indexes().len())?;
            let number = u32::try_from(number).map_err(|_| InspectError::StructureLimit)?;
            let first = checked
                .indexes()
                .iter()
                .find(|index| index.index().data_number() == number)
                .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
            let index = first.index();
            for alias in checked
                .indexes()
                .iter()
                .filter(|alias| alias.index().data_number() == number)
            {
                reader.tick()?;
                if alias.status() != IndexCheckStatus::Checked
                    || alias.index().root_page() != index.root_page()
                    || alias.index().usage_map() != index.usage_map()
                    || alias.index().columns() != index.columns()
                    || alias
                        .index()
                        .constraints()
                        .map(|flags| (flags.unique(), flags.required(), flags.ignore_nulls()))
                        != index
                            .constraints()
                            .map(|flags| (flags.unique(), flags.required(), flags.ignore_nulls()))
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
            if index
                .constraints()
                .is_some_and(|flags| flags.ignore_nulls())
                && checked.indexes().iter().any(|alias| {
                    alias.index().data_number() == number
                        && alias
                            .index()
                            .constraints()
                            .is_some_and(|flags| flags.primary_key())
                })
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let owned = allocation::pages(reader, index.usage_map())?;
            let mut reached = Vec::new();
            visit(index.root_page(), definition, 0, &mut reached, reader)?;
            let levels = reached.len().checked_ilog2().unwrap_or(0) as usize + 1;
            reader.work(reached.len().saturating_mul(levels))?;
            reached.sort_unstable();
            if reached != owned {
                return Err(EditError::Preservation);
            }
            for page in owned {
                reader.tick()?;
                reader.work(source_pages.len())?;
                if source_pages.contains(&page) {
                    return Err(EditError::Preservation);
                }
                push(&mut source_pages, page, reader)?;
            }
            push(
                &mut owners,
                Owner::IndexOwned {
                    definition,
                    physical: number,
                },
                reader,
            )?;
            push(
                &mut physical,
                Physical {
                    checked,
                    index,
                    updates: input.updates,
                },
                reader,
            )?;
        }
        reader.work(checked.indexes().len())?;
        if checked
            .indexes()
            .iter()
            .any(|index| index.index().data_number() as usize >= schema.layout.physical_indexes)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    Ok(Source {
        physical,
        owners,
        index_pages: source_pages,
    })
}

fn visit(
    number: u32,
    definition: u32,
    depth: usize,
    reached: &mut Vec<u32>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    reader.work(reached.len())?;
    if depth >= MAX_INDEX_DEPTH || reached.contains(&number) {
        return Err(EditError::Preservation);
    }
    push(reached, number, reader)?;
    let raw = reader.page(number)?;
    let header = IndexHeader::read(&raw)?;
    if !matches!(header.kind, LEAF | NODE)
        || header.version != 1
        || header.owner.get() != definition
    {
        return Err(EditError::Preservation);
    }
    if header.kind == LEAF {
        return Ok(());
    }
    let mut previous = 0usize;
    let mut first_end = None;
    let mut children = Vec::new();
    let active = PAGE_BYTES
        .checked_sub(usize::from(header.free.get()))
        .and_then(|end| end.checked_sub(ENTRY_START))
        .ok_or(InspectError::Malformed)?;
    for end in 0..=(PAGE_BYTES - ENTRY_START) {
        reader.tick()?;
        if raw[MASK_START + end / u8::BITS as usize] & (1 << (end % u8::BITS as usize)) == 0 {
            continue;
        }
        if end <= previous || end > active {
            return Err(InspectError::Malformed.into());
        }
        let shared = if first_end.is_some() {
            usize::from(header.prefix.get())
        } else {
            0
        };
        let length = shared
            .checked_add(end - previous)
            .ok_or(InspectError::Malformed)?;
        let child_start = length
            .checked_sub(CHILD_BYTES)
            .ok_or(InspectError::Malformed)?;
        if first_end.is_some_and(|first| shared > first) {
            return Err(InspectError::Malformed.into());
        }
        let mut bytes = [0; CHILD_BYTES];
        for (position, byte) in bytes.iter_mut().enumerate() {
            let offset = child_start + position;
            *byte = if offset < shared {
                raw[ENTRY_START + offset]
            } else {
                raw[ENTRY_START + previous + offset - shared]
            };
        }
        push(&mut children, u32::from_be_bytes(bytes), reader)?;
        first_end.get_or_insert(end);
        previous = end;
    }
    if header.tail.get() != 0 {
        push(&mut children, header.tail.get(), reader)?;
    }
    if children.is_empty() {
        return Err(InspectError::Malformed.into());
    }
    for child in children {
        visit(child, definition, depth + 1, reached, reader)?;
    }
    Ok(())
}
