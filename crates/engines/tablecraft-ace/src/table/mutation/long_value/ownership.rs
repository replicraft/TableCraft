//! Writable ownership additionally checks other catalog declarations and live refs.
use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, NativeImage, NativeTableStorage, TableSchema,
    format::{
        allocation,
        bytes::u32_at,
        data_type::{LONG_TEXT, OLE},
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, live_row_range, row_count},
        row::{MASK_BITS, reference_page},
    },
    header::check,
    inspect_table_schema,
    table::{
        indexes,
        long_value::{maps, trace},
        record_layout,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

mod profile;

struct Claim {
    page: u32,
    definition: u32,
    column: usize,
}

pub(super) struct Inventory {
    claims: Vec<Claim>,
    live: Vec<Claim>,
    protected: Vec<u32>,
}

fn append_claim(
    claims: &mut Vec<Claim>,
    page: u32,
    schema: &TableSchema,
    column: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    reader
        .budget
        .charge_allocation(1, size_of::<Claim>() as u64)
        .map_err(InspectError::from)?;
    claims
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    claims.push(Claim {
        page,
        definition: schema.definition_page(),
        column,
    });

    Ok(())
}

fn protect(
    protected: &mut Vec<u32>,
    page: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(protected.len())?;
    if !protected.contains(&page) {
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        protected
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        protected.push(page);
    }

    Ok(())
}

impl Inventory {
    pub fn capture(
        image: &NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let mut result = Self {
            claims: Vec::new(),
            live: Vec::new(),
            protected: Vec::new(),
        };
        let catalog = image.catalog();
        let scope = catalog.catalog().scope();
        for origin in catalog.origins() {
            check(budget, cancelled)?;
            if origin.table_storage() != Some(NativeTableStorage::Local) {
                continue;
            }
            budget
                .charge(Resource::Steps, catalog.catalog().objects().len() as u64)
                .map_err(InspectError::from)?;
            let object = catalog
                .catalog()
                .objects()
                .iter()
                .find(|object| object.id() == origin.object_id())
                .ok_or(InspectError::Malformed)?;
            let schema = inspect_table_schema(
                &mut image.reader(),
                scope,
                object.name().as_str(),
                budget,
                cancelled,
            )?;
            let mut source = image.reader();
            let mut reader = Reader {
                source: &mut source,
                header: catalog.header(),
                budget,
                cancelled,
                reads: 0,
            };
            result.table(&schema, &mut reader)?;
        }

        Ok(result)
    }

    fn table(
        &mut self,
        schema: &TableSchema,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        protect(&mut self.protected, schema.definition_page(), reader)?;
        protect(
            &mut self.protected,
            reference_page(schema.layout.usage),
            reader,
        )?;
        let ordinary = allocation::pages(reader, schema.layout.usage)?;
        for &page in &ordinary {
            protect(&mut self.protected, page, reader)?;
        }
        let metadata = definition::read(reader, schema.definition_page())?;
        let hint_reference = u32_at(&metadata, TABLE_FREE_MAP_OFFSET)?;
        if hint_reference != 0 {
            protect(&mut self.protected, reference_page(hint_reference), reader)?;
        }
        let indexes = indexes::definitions_with_tail(reader, schema)?;
        for index in indexes.indexes {
            reader.tick()?;
            protect(
                &mut self.protected,
                reference_page(index.usage_map()),
                reader,
            )?;
            for page in allocation::pages(reader, index.usage_map())? {
                protect(&mut self.protected, page, reader)?;
            }
        }

        let long_fields = schema
            .columns()
            .iter()
            .any(|field| matches!(field.native_type(), LONG_TEXT | OLE));
        // A table without surviving long fields can still retain historical
        // column-owned orphan storage. Its declaration tail is not inferred.
        let declarations = maps::declarations(reader, schema)?;
        if !declarations.complete {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        for declaration in declarations.entries {
            reader.tick()?;
            protect(
                &mut self.protected,
                reference_page(declaration.references.owned()),
                reader,
            )?;
            protect(
                &mut self.protected,
                reference_page(declaration.references.candidate()),
                reader,
            )?;
            for page in allocation::pages(reader, declaration.references.owned())? {
                append_claim(&mut self.claims, page, schema, declaration.number, reader)?;
            }
        }
        if !long_fields {
            return Ok(());
        }

        let mut rows = 0_u32;
        for number in ordinary {
            let page = reader.page(number)?;
            if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || u32_at(&page, OWNER_OFFSET)? != schema.definition_page()
            {
                return Err(InspectError::Malformed.into());
            }
            for slot in 0..row_count(&page)? {
                reader.tick()?;
                if live_row_range(&page, slot)?.is_none() {
                    continue;
                }
                reader
                    .budget
                    .charge(Resource::Rows, 1)
                    .map_err(InspectError::from)?;
                let slot = u8::try_from(slot).map_err(|_| InspectError::Malformed)?;
                let row = reader.row_location(number, slot, schema.definition_page(), &page)?;
                let layout = record_layout::parse_values(&row.data, schema, reader.budget)?;
                for field in schema
                    .columns()
                    .iter()
                    .filter(|field| matches!(field.native_type(), LONG_TEXT | OLE))
                {
                    reader.tick()?;
                    if field.number() >= layout.count {
                        continue;
                    }
                    let mask = row
                        .data
                        .get(layout.mask_start + field.number() / MASK_BITS)
                        .ok_or(InspectError::Malformed)?;
                    if mask & (1 << (field.number() % MASK_BITS)) == 0 {
                        continue;
                    }
                    if !profile::readable(field) {
                        return Err(EditError::Unsupported(WriteBlocker::Column));
                    }
                    let start = *layout
                        .offsets
                        .get(field.field.variable)
                        .ok_or(InspectError::Malformed)?;
                    let end = *layout
                        .offsets
                        .get(field.field.variable + 1)
                        .ok_or(InspectError::Malformed)?;
                    let descriptor = row.data.get(start..end).ok_or(InspectError::Malformed)?;
                    let traced = trace::read(reader, descriptor)?;
                    for chunk in traced.chunks {
                        append_claim(
                            &mut self.live,
                            reference_page(chunk.reference),
                            schema,
                            field.number(),
                            reader,
                        )?;
                    }
                }
                rows = rows.checked_add(1).ok_or(InspectError::StructureLimit)?;
            }
        }
        if rows != schema.declared_rows() {
            return Err(InspectError::Malformed.into());
        }

        Ok(())
    }

    pub fn require_exclusive(
        &self,
        schema: &TableSchema,
        column: usize,
        pages: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for &page in pages {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (self.protected.len() + self.claims.len() + self.live.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if self.protected.contains(&page)
                || self.claims.iter().chain(&self.live).any(|claim| {
                    claim.page == page
                        && (claim.definition != schema.definition_page() || claim.column != column)
                })
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }

        Ok(())
    }
}
