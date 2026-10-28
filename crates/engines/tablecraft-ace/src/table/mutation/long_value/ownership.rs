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
            maps: None,
        };
        let maps = map_authority::capture_catalog(
            image,
            budget,
            cancelled,
            |schema, indexes, declarations, reader| {
                result.table(schema, indexes, declarations, reader)
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

        Ok(result)
    }

    fn table(
        &mut self,
        schema: &TableSchema,
        indexes: &[NativeIndex],
        declarations: &storage_maps::Declarations,
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
