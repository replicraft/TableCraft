//! Native-count creation separates logical definitions from physical storage.
pub(in crate::creation) mod topology;

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "journal"))]
mod indirect_tests;

use super::{
    CreateDatabaseError, build, definition,
    profile::{self, Fields},
    record::{self, DataPage},
    request::Request,
};
use crate::{
    InspectError, NativeSortOrder,
    format::{
        page::{PAGE_BYTES, TABLE_DEFINITION_SIGNATURE, USAGE_SIGNATURE},
        row::reference,
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use topology::{BITMAP_OFFSET, BITMAP_PAGES, CHAIN_PAYLOAD, DEFINITION_HEADER, Topology};

const EXTERNAL_DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const PROPERTY_OWNED_SLOT: usize = 16;
const PROPERTY_CANDIDATE_SLOT: usize = 17;

fn zeroed(
    length: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, CreateDatabaseError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    while bytes.len() < length {
        check(budget, cancelled)?;
        let end = length.min(bytes.len() + PAGE_BYTES);
        budget
            .charge(Resource::Steps, (end - bytes.len()) as u64)
            .map_err(InspectError::from)?;
        bytes.resize(end, 0);
    }
    Ok(bytes)
}

/// Fresh native construction starts from authored systems, not an imported image.
pub(super) fn first(
    request: &Request<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, CreateDatabaseError> {
    let length =
        definition::length_with_primary(request.fields(), false, request.primary.as_ref())?;
    let body_length = request
        .properties
        .as_ref()
        .map_or(0, |body| body.body_length());
    let topology = Topology::capture(
        profile::USER_DEFINITION,
        length,
        body_length,
        request.primary.is_some(),
    )?;
    let mut bytes = build::initialize(topology.end * PAGE_BYTES, budget, cancelled)?;

    for (number, fields, rows, slot) in [
        (
            profile::CATALOG_DEFINITION,
            Fields::new(&profile::CATALOG_FIELDS),
            profile::CATALOG_OBJECT_COUNT,
            profile::CATALOG_MAP_SLOT,
        ),
        (
            profile::RELATIONSHIPS_DEFINITION,
            Fields::new(&profile::RELATIONSHIP_FIELDS),
            0,
            profile::RELATIONSHIPS_MAP_SLOT,
        ),
        (
            profile::ACES_DEFINITION,
            Fields::new(&profile::ACE_FIELDS),
            0,
            profile::ACES_MAP_SLOT,
        ),
    ] {
        definition::write(
            build::page(&mut bytes, number)?,
            fields,
            rows,
            slot,
            budget,
            cancelled,
        )?;
    }

    write_table(
        &mut bytes,
        request,
        &topology,
        MapReferences {
            owned: reference(profile::MAP_PAGE as u32, profile::USER_MAP_SLOT as u8),
            candidate: reference(profile::MAP_PAGE as u32, profile::USER_CANDIDATE_SLOT as u8),
            index: topology
                .index
                .map(|_| reference(profile::MAP_PAGE as u32, profile::MAP_ROWS as u8)),
        },
        Some(profile::GENERAL_SORT),
        budget,
        cancelled,
    )?;
    {
        let mut catalog = DataPage::new(
            build::page(&mut bytes, profile::CATALOG_DATA)?,
            profile::CATALOG_DEFINITION,
        )?;
        for object in build::systems() {
            record::catalog(
                catalog.append(record::catalog_length(object.name), budget, cancelled)?,
                &object,
                budget,
                cancelled,
            )?;
        }
        let user = profile::CatalogObject {
            id: profile::USER_DEFINITION as u32,
            parent: profile::TABLES_ID,
            name: request.catalog_name,
            system: false,
        };
        let length = record::catalog_length(user.name)
            + if body_length == 0 {
                0
            } else {
                EXTERNAL_DESCRIPTOR_BYTES
            };
        let destination = catalog.append(length, budget, cancelled)?;
        if body_length == 0 {
            record::catalog(destination, &user, budget, cancelled)?;
        } else {
            record::catalog_with_linked_properties(
                destination,
                &user,
                reference(topology.properties.start as u32, 0),
                body_length,
                budget,
                cancelled,
            )?;
        }
        catalog.finish()?;
    }
    write_maps(
        &mut bytes,
        &topology,
        request.minimum_row_fits(),
        budget,
        cancelled,
    )?;
    // All authored pages are allocated. The free-page bitmap represents the
    // actual output extent, growing its real row instead of imposing page 63.
    {
        let length = 5 + 8_usize.max(topology.end.div_ceil(u8::BITS as usize));
        let mut global = DataPage::new(
            build::page(&mut bytes, profile::GLOBAL_PAGE)?,
            profile::GLOBAL_PAGE,
        )?;
        global.append(length, budget, cancelled)?;
        global.finish()?;
    }
    check(budget, cancelled)?;
    Ok(bytes)
}

pub(in crate::creation) struct MapReferences {
    pub owned: u32,
    pub candidate: u32,
    pub index: Option<u32>,
}

pub(in crate::creation) fn write_table(
    bytes: &mut [u8],
    request: &Request<'_>,
    topology: &Topology,
    maps: MapReferences,
    text_sort_order: Option<NativeSortOrder>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let primary = match (request.primary.as_ref(), topology.index, maps.index) {
        (Some(prepared), Some(root), Some(map)) => Some(definition::PrimaryRole {
            prepared,
            root,
            map,
        }),
        (None, None, None) => None,
        _ => return Err(CreateDatabaseError::Layout),
    };
    let length =
        definition::length_with_primary(request.fields(), false, request.primary.as_ref())?;
    let mut logical = zeroed(length, budget, cancelled)?;
    definition::write_logical_role(
        &mut logical,
        request.fields(),
        0,
        definition::Role {
            text_sort_order,
            user: true,
            catalog: false,
            owned: maps.owned,
            candidate: maps.candidate,
        },
        primary,
        budget,
        cancelled,
    )?;
    pack_definition(bytes, &logical, topology, budget, cancelled)?;
    DataPage::new(
        build::page(bytes, topology.data)?,
        topology.definition.start,
    )?
    .finish()?;
    if let Some((primary, index)) = request.primary.as_ref().zip(topology.index) {
        primary.write_leaf(
            build::page(bytes, index)?,
            topology.definition.start,
            topology.data,
            budget,
            cancelled,
        )?;
    }
    if let Some(properties) = &request.properties {
        let mut body = zeroed(properties.body_length(), budget, cancelled)?;
        properties.write_body(&mut body, budget, cancelled)?;
        write_properties(bytes, &body, topology, budget, cancelled)?;
    }
    Ok(())
}

fn pack_definition(
    bytes: &mut [u8],
    logical: &[u8],
    topology: &Topology,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let mut cursor = 0;
    for number in topology.definition.start..topology.definition.end {
        check(budget, cancelled)?;
        let header = if number == topology.definition.start {
            0
        } else {
            DEFINITION_HEADER
        };
        let take = (PAGE_BYTES - header).min(logical.len() - cursor);
        let page = build::page(bytes, number)?;
        page[..TABLE_DEFINITION_SIGNATURE.len()].copy_from_slice(&TABLE_DEFINITION_SIGNATURE);
        budget
            .charge(Resource::Steps, take as u64)
            .map_err(InspectError::from)?;
        page[header..header + take].copy_from_slice(&logical[cursor..cursor + take]);
        cursor += take;
        let next = if cursor == logical.len() {
            0
        } else {
            number + 1
        };
        definition::put32(
            page,
            4,
            u32::try_from(next).map_err(|_| CreateDatabaseError::Layout)?,
        )?;
        definition::put16(
            page,
            2,
            PAGE_BYTES.saturating_sub(header + take + DEFINITION_HEADER),
        )?;
    }
    if cursor != logical.len() {
        return Err(CreateDatabaseError::Layout);
    }
    Ok(())
}

fn write_properties(
    bytes: &mut [u8],
    body: &[u8],
    topology: &Topology,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let mut cursor = 0;
    for number in topology.properties.start..topology.properties.end {
        let take = CHAIN_PAYLOAD.min(body.len() - cursor);
        let next = if cursor + take == body.len() {
            0
        } else {
            reference((number + 1) as u32, 0)
        };
        let mut page = DataPage::new(
            build::page(bytes, number)?,
            u32::from_le_bytes(*b"LVAL") as usize,
        )?;
        let record = page.append(size_of::<u32>() + take, budget, cancelled)?;
        definition::put32(record, 0, next)?;
        record[size_of::<u32>()..].copy_from_slice(&body[cursor..cursor + take]);
        page.finish()?;
        cursor += take;
    }
    if cursor != body.len() {
        return Err(CreateDatabaseError::Layout);
    }
    Ok(())
}

fn write_maps(
    bytes: &mut [u8],
    topology: &Topology,
    data_candidate: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    for (block, number) in (topology.bitmaps.start..topology.bitmaps.end).enumerate() {
        let page = build::page(bytes, number)?;
        page[..USAGE_SIGNATURE.len()].copy_from_slice(&USAGE_SIGNATURE);
        for member in topology.properties.start..topology.properties.end {
            check(budget, cancelled)?;
            if member / BITMAP_PAGES == block {
                let bit = member % BITMAP_PAGES;
                page[BITMAP_OFFSET + bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    let mut maps = DataPage::new(build::page(bytes, profile::MAP_PAGE)?, 0)?;
    for slot in 0..profile::MAP_ROWS + usize::from(topology.index.is_some()) {
        let length = if slot == PROPERTY_OWNED_SLOT {
            topology.owned_property_map_length()
        } else {
            profile::MAP_RECORD_BYTES
        };
        let record = maps.append(length, budget, cancelled)?;
        if slot == PROPERTY_OWNED_SLOT && !topology.bitmaps.is_empty() {
            record[0] = 1;
            for (index, number) in (topology.bitmaps.start..topology.bitmaps.end).enumerate() {
                definition::put32(record, 1 + index * size_of::<u32>(), number as u32)?;
            }
        } else if slot == PROPERTY_OWNED_SLOT {
            for member in topology.properties.start..topology.properties.end {
                record[5 + member / 8] |= 1 << (member % 8);
            }
        } else {
            let member = match slot {
                profile::CATALOG_MAP_SLOT | profile::CATALOG_CANDIDATE_SLOT => {
                    Some(profile::CATALOG_DATA)
                }
                profile::USER_MAP_SLOT => Some(topology.data),
                profile::USER_CANDIDATE_SLOT if data_candidate => Some(topology.data),
                PROPERTY_CANDIDATE_SLOT => topology.candidate,
                profile::MAP_ROWS => topology.index,
                _ => None,
            };
            if let Some(member) = member {
                let base = member / 64 * 64;
                definition::put32(record, 1, base as u32)?;
                let bit = member - base;
                record[5 + bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    maps.finish()
}
