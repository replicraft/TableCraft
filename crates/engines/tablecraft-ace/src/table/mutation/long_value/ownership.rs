//! Writable ownership additionally checks other catalog declarations and live refs.
mod profile;
#[cfg(test)]
mod tests;

use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, NativeImage, NativeIndex, TableSchema,
    format::{
        allocation,
        bytes::u32_at,
        data_type::{LONG_TEXT, OLE},
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, live_row_range, row_count},
        row::{MASK_BITS, reference_page},
    },
    header::check,
    table::{
        long_value::{maps as storage_maps, trace},
        mutation::map_authority::{self, Census, Owner, RecordIdentity},
        record_layout,
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Claim {
    page: u32,
    definition: u32,
    column: usize,
}

#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum ColumnMapKind {
    Owned,
    Candidate,
}

pub(in crate::table::mutation) struct Inventory {
    claims: Vec<Claim>,
    live: Vec<Claim>,
    protected: Vec<u32>,
    maps: Option<Census>,
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

pub(in crate::table::mutation) struct ZeroPair {
    pub column: usize,
    pub offset: usize,
}

pub(in crate::table::mutation) struct MissingDeclaration {
    pub column: usize,
}

pub(in crate::table::mutation) struct InitializationSources {
    pub zeros: Vec<ZeroPair>,
    pub missing: Vec<MissingDeclaration>,
    pub tail: Option<storage_maps::DefinitionTail>,
}

impl Inventory {
    pub fn capture(
        image: &NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_inner(image, None, &[], budget, cancelled).map(|(inventory, _)| inventory)
    }

    pub(in crate::table::mutation) fn capture_for_insert(
        image: &NativeImage,
        schema: &TableSchema,
        zero_columns: &[usize],
        external_columns: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, InitializationSources), EditError> {
        Self::capture_inner(
            image,
            Some((schema.definition_page(), zero_columns)),
            external_columns,
            budget,
            cancelled,
        )
    }

    fn capture_inner(
        image: &NativeImage,
        initializing: Option<(u32, &[usize])>,
        external_columns: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, InitializationSources), EditError> {
        let mut result = Self {
            claims: Vec::new(),
            live: Vec::new(),
            protected: Vec::new(),
            maps: None,
        };
        let mut zeros = Vec::new();
        let mut missing = Vec::new();
        let mut tail = None;
        let maps = map_authority::capture_catalog_for_initialization(
            image,
            initializing,
            budget,
            cancelled,
            |schema, indexes, declarations, reader| {
                let selected =
                    initializing.filter(|(definition, _)| *definition == schema.definition_page());
                let mut empty_columns = Vec::new();
                for declaration in &declarations.entries {
                    reader.tick()?;
                    if declaration.references.owned() != 0
                        || declaration.references.candidate() != 0
                    {
                        continue;
                    }
                    let Some((_, columns)) = selected else {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    };
                    reader.work(columns.len())?;
                    if !columns.contains(&declaration.number) {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    reader
                        .budget
                        .charge_allocation(1, size_of::<usize>() as u64)
                        .map_err(InspectError::from)?;
                    empty_columns
                        .try_reserve(1)
                        .map_err(|_| InspectError::Allocation)?;
                    empty_columns.push(declaration.number);
                    reader
                        .budget
                        .charge_allocation(1, size_of::<ZeroPair>() as u64)
                        .map_err(InspectError::from)?;
                    zeros.try_reserve(1).map_err(|_| InspectError::Allocation)?;
                    zeros.push(ZeroPair {
                        column: declaration.number,
                        offset: storage_maps::zero_pair_offset(reader, schema, declaration.number)?,
                    });
                }
                if selected.is_some() {
                    for field in schema.columns() {
                        reader.tick()?;
                        reader.work(external_columns.len() + declarations.entries.len())?;
                        if !external_columns.contains(&field.number())
                            || declarations
                                .entries
                                .iter()
                                .any(|entry| entry.number == field.number())
                        {
                            continue;
                        }
                        if !matches!(field.native_type(), LONG_TEXT | OLE) {
                            return Err(EditError::Unsupported(WriteBlocker::Column));
                        }
                        if tail.is_none() {
                            tail = Some(storage_maps::definition_tail(reader, schema).map_err(
                                |error| match error {
                                    InspectError::Unsupported => {
                                        EditError::Unsupported(WriteBlocker::RowLayout)
                                    }
                                    other => other.into(),
                                },
                            )?);
                        }
                        reader
                            .budget
                            .charge_allocation(1, size_of::<MissingDeclaration>() as u64)
                            .map_err(InspectError::from)?;
                        missing
                            .try_reserve(1)
                            .map_err(|_| InspectError::Allocation)?;
                        missing.push(MissingDeclaration {
                            column: field.number(),
                        });
                        reader
                            .budget
                            .charge_allocation(1, size_of::<usize>() as u64)
                            .map_err(InspectError::from)?;
                        empty_columns
                            .try_reserve(1)
                            .map_err(|_| InspectError::Allocation)?;
                        empty_columns.push(field.number());
                    }
                }
                result.table(schema, indexes, declarations, &empty_columns, reader)
            },
        )?;
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        maps.protect_storage(&mut result.protected, &mut reader)?;
        result.maps = Some(maps);
        Ok((
            result,
            InitializationSources {
                zeros,
                missing,
                tail,
            },
        ))
    }

    fn table(
        &mut self,
        schema: &TableSchema,
        indexes: &[NativeIndex],
        declarations: &storage_maps::Declarations,
        empty_columns: &[usize],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        protect(&mut self.protected, schema.definition_page(), reader)?;
        let ordinary = allocation::pages(reader, schema.layout.usage)?;
        for &page in &ordinary {
            protect(&mut self.protected, page, reader)?;
        }
        for index in indexes {
            reader.tick()?;
            for page in allocation::pages(reader, index.usage_map())? {
                protect(&mut self.protected, page, reader)?;
            }
        }

        let long_fields = schema
            .columns()
            .iter()
            .any(|field| matches!(field.native_type(), LONG_TEXT | OLE));
        for declaration in &declarations.entries {
            reader.tick()?;
            reader.work(empty_columns.len())?;
            if empty_columns.contains(&declaration.number) {
                continue;
            }
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
                    reader.work(empty_columns.len())?;
                    if empty_columns.contains(&field.number())
                        && !matches!(traced.kind, trace::Kind::Null | trace::Kind::Inline)
                    {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
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

    pub(in crate::table::mutation) fn require_record_rewrite(
        &self,
        schema: &TableSchema,
        column: usize,
        kind: ColumnMapKind,
        record: (u32, Range<usize>),
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let definition = schema.definition_page();
        let owner = match kind {
            ColumnMapKind::Owned => Owner::ColumnOwned { definition, column },
            ColumnMapKind::Candidate => Owner::ColumnCandidate { definition, column },
        };
        self.maps
            .as_ref()
            .ok_or(InspectError::Malformed)?
            .require_record_rewrite(
                &[Owner::GlobalFree, owner],
                RecordIdentity {
                    owner,
                    reference: record.0,
                    range: record.1,
                },
                budget,
                cancelled,
            )
    }

    pub fn require_exclusive(
        &self,
        schema: &TableSchema,
        column: usize,
        pages: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let definition = schema.definition_page();
        let selected = [
            Owner::GlobalFree,
            Owner::OrdinaryOwned { definition },
            Owner::OrdinaryCandidate { definition },
            Owner::ColumnOwned { definition, column },
            Owner::ColumnCandidate { definition, column },
        ];
        self.maps
            .as_ref()
            .ok_or(InspectError::Malformed)?
            .require_exclusive(&selected, budget, cancelled)?;

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
