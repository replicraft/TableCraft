//! Independently resolve full requests into LVAL pages and descriptor authority.
mod choice;
mod cursor;
mod initialization;
mod maps;
mod tail;

use self::{choice::Storage, cursor::Cursor as Placement};
use super::records::payload::long_value::Payload;
use super::{ImageProof, reserve};
use crate::{
    EditError, InspectError, LongValueCheckStatus, LongValueMapReferences, NativeImage, TableCheck,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        reader::Reader,
    },
    table::long_value::{check_table_long_values_for_initialization, maps as source_maps},
    table::mutation::{
        insert::{RowInsert, Staged},
        long_value::ownership::Inventory,
    },
};
use std::io::Cursor;

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const INLINE_TAG: u32 = 0x8000_0000;
const SINGLE_TAG: u32 = 0x4000_0000;
const SINGLE_BYTES: usize = 4076;
const LINK_BYTES: usize = size_of::<u32>();
const CHAIN_BYTES: usize = SINGLE_BYTES - LINK_BYTES;

struct Declaration {
    position: usize,
    references: LongValueMapReferences,
    initialized: Option<u32>,
}

pub(super) struct Resolved {
    pub column: usize,
    pub descriptor: Vec<u8>,
}

pub(super) struct ResolvedRow {
    pub fields: Vec<Resolved>,
    pub record_length: usize,
}

pub(super) fn derive(
    image: &NativeImage,
    checked: &TableCheck,
    requests: &[RowInsert],
    staged: &Staged,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Option<Vec<ResolvedRow>>, EditError> {
    if !checked.schema().columns().iter().any(|field| {
        matches!(
            field.native_type(),
            crate::format::data_type::LONG_TEXT | crate::format::data_type::OLE
        )
    }) {
        if !staged.long_values.pages.is_empty()
            || !staged.long_values.metadata.is_empty()
            || !staged.long_values.initializations.is_empty()
            || !staged.long_values.qualified_zero_columns.is_empty()
            || !staged.long_values.qualified_missing_columns.is_empty()
            || staged.long_values.definition_growth.is_some()
            || staged
                .data
                .append
                .long_value_map_containers()
                .next()
                .is_some()
            || staged.data.append.long_values().next().is_some()
            || staged
                .data
                .append
                .long_value_owned_bitmaps()
                .next()
                .is_some()
        {
            return Err(EditError::Preservation);
        }
        return Ok(None);
    }
    let rows = choice::derive(checked.schema(), requests, reader)?;
    let mut selected = Vec::new();
    for row in &rows {
        for field in &row.fields {
            reader.tick()?;
            if field.storage != Storage::Inline {
                reader.work(selected.len())?;
                if !selected.contains(&field.column) {
                    reserve(&mut selected, 1, reader)?;
                    selected.push(field.column);
                }
            }
        }
    }
    let schema = checked.schema();
    let mut native_columns = Vec::new();
    reserve(&mut native_columns, schema.columns().len(), reader)?;
    for field in schema.columns() {
        reader.tick()?;
        if matches!(
            field.native_type(),
            crate::format::data_type::LONG_TEXT | crate::format::data_type::OLE
        ) {
            native_columns.push(field.number());
        }
    }

    // Preserve the existing inline-only fast path. A declaration read detects
    // an actual zero pair before the exceptional source payload census runs.
    let needs_inventory = if selected.is_empty() {
        let declarations = source_maps::declarations(reader, schema)?;
        reader.work(declarations.entries.len())?;
        declarations
            .entries
            .iter()
            .any(|entry| entry.references.owned() == 0 && entry.references.candidate() == 0)
    } else {
        true
    };
    let mut initializations = Vec::new();
    let mut growth = None;
    let mut qualified_missing = Vec::new();
    let mut placement = if needs_inventory {
        let mut external_columns = Vec::new();
        reserve(&mut external_columns, selected.len(), reader)?;
        for &position in &selected {
            reader.tick()?;
            external_columns.push(schema.columns()[position].number());
        }
        let (inventory, sources) = Inventory::capture_for_insert(
            image,
            schema,
            &native_columns,
            &external_columns,
            reader.budget,
            reader.cancelled,
        )?;
        reserve(&mut qualified_missing, sources.missing.len(), reader)?;
        for missing in &sources.missing {
            reader.tick()?;
            qualified_missing.push(missing.column);
        }
        reserve(
            &mut proof.qualified_zero_columns,
            sources.zeros.len(),
            reader,
        )?;
        for zero in &sources.zeros {
            reader.tick()?;
            proof.qualified_zero_columns.push(zero.column);
        }
        if selected.is_empty() {
            None
        } else {
            let report = check_table_long_values_for_initialization(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                schema.name(),
                &native_columns,
                reader.budget,
                reader.cancelled,
            )?;
            if !schema.same_definition(report.schema()) {
                return Err(EditError::Preservation);
            }
            let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
            global.require_allocated_storage(reader, &global)?;
            initializations =
                initialization::derive(schema, &selected, &sources, &global, proof, reader)?;
            growth = tail::derive(
                schema,
                sources.tail.as_ref(),
                &initializations,
                proof,
                reader,
            )?;
            let mut declarations = Vec::new();
            reserve(&mut declarations, selected.len(), reader)?;
            for position in selected {
                reader.tick()?;
                reader.work(initializations.len())?;
                if let Some(initialized) = initializations
                    .iter()
                    .find(|entry| entry.position == position)
                {
                    declarations.push(Declaration {
                        position,
                        references: initialized.references,
                        initialized: Some(initialized.number),
                    });
                    continue;
                }
                let field = &schema.columns()[position];
                let mut matched = None;
                for stored in report.columns() {
                    reader.tick()?;
                    if stored.column_number() == field.number() {
                        if matched.is_some()
                            || stored.status() != LongValueCheckStatus::Checked
                            || stored.shared_references() != 0
                            || stored.shared_owned_pages() != 0
                        {
                            return Err(EditError::Preservation);
                        }
                        matched = Some(stored.maps().ok_or(EditError::Preservation)?);
                    }
                }
                declarations.push(Declaration {
                    position,
                    references: matched.ok_or(EditError::Preservation)?,
                    initialized: None,
                });
            }
            Some(Placement::capture(
                schema,
                &declarations,
                proof,
                inventory,
                reader,
            )?)
        }
    } else {
        None
    };
    reader.work(proof.qualified_zero_columns.len())?;
    reader.work(qualified_missing.len())?;
    if proof.qualified_zero_columns != staged.long_values.qualified_zero_columns
        || qualified_missing != staged.long_values.qualified_missing_columns
    {
        return Err(EditError::Preservation);
    }
    let mut resolved = Vec::new();
    reserve(&mut resolved, rows.len(), reader)?;
    for row in rows {
        let mut fields = Vec::new();
        reserve(&mut fields, row.fields.len(), reader)?;
        for field in row.fields {
            reader.tick()?;
            let payload = match field.payload {
                Payload::Null => {
                    if field.storage != Storage::Inline {
                        return Err(EditError::Preservation);
                    }
                    fields.push(Resolved {
                        column: field.column,
                        descriptor: Vec::new(),
                    });
                    continue;
                }
                Payload::Present(bytes) => bytes,
            };
            let linked = field.storage == Storage::Chained;
            let mut locations = Vec::new();
            if field.storage != Storage::Inline {
                if payload.is_empty() {
                    return Err(EditError::Preservation);
                }
                let capacity = if linked { CHAIN_BYTES } else { SINGLE_BYTES };
                reserve(&mut locations, payload.len().div_ceil(capacity), reader)?;
                for part in payload.chunks(capacity) {
                    let mut body = Vec::new();
                    let prefix = if linked { LINK_BYTES } else { 0 };
                    reserve(&mut body, prefix + part.len(), reader)?;
                    reader.work(prefix + part.len())?;
                    body.resize(prefix, 0);
                    body.extend_from_slice(part);
                    locations.push(placement.as_mut().ok_or(EditError::Preservation)?.append(
                        field.column,
                        &body,
                        proof,
                        reader,
                    )?);
                }
                if linked {
                    for (position, location) in locations.iter().enumerate() {
                        reader.tick()?;
                        let next = locations.get(position + 1).map_or(0, |next| next.reference);
                        proof.bytes[location.range.start..location.range.start + LINK_BYTES]
                            .copy_from_slice(&next.to_le_bytes());
                    }
                }
            }
            let inline = field.storage == Storage::Inline;
            let tag = match field.storage {
                Storage::Inline => INLINE_TAG,
                Storage::Single => SINGLE_TAG,
                Storage::Chained => 0,
            };
            let length = DESCRIPTOR_BYTES
                .checked_add(if inline { payload.len() } else { 0 })
                .ok_or(InspectError::StructureLimit)?;
            let mut descriptor = Vec::new();
            reserve(&mut descriptor, length, reader)?;
            reader.work(length)?;
            let count = u32::try_from(payload.len()).map_err(|_| InspectError::StructureLimit)?;
            descriptor.extend_from_slice(&(tag | count).to_le_bytes());
            descriptor.extend_from_slice(
                &locations
                    .first()
                    .map_or(0, |first| first.reference)
                    .to_le_bytes(),
            );
            descriptor.extend_from_slice(&0_u32.to_le_bytes());
            if inline {
                descriptor.extend_from_slice(&payload);
            }
            fields.push(Resolved {
                column: field.column,
                descriptor,
            });
        }
        resolved.push(ResolvedRow {
            fields,
            record_length: row.record_length,
        });
    }
    if let Some(placement) = placement {
        placement.compare(&staged.long_values, proof, reader)?;
        placement.require_purposes(staged, reader)?;
    } else if !staged.long_values.pages.is_empty()
        || !staged.long_values.metadata.is_empty()
        || staged.data.append.long_values().next().is_some()
        || staged
            .data
            .append
            .long_value_owned_bitmaps()
            .next()
            .is_some()
    {
        return Err(EditError::Preservation);
    }
    initialization::compare(&initializations, staged, proof, reader)?;
    tail::compare(
        growth.as_ref(),
        staged.long_values.definition_growth.as_ref(),
        reader,
    )?;
    Ok(Some(resolved))
}
