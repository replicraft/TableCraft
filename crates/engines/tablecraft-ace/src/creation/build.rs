//! One paid image allocation followed by direct, bounded serialization.
use super::{
    CreateDatabaseError, definition,
    profile::{self, Fields},
    record::{self, DataPage},
    request::Request,
    stored_text::Plan,
};
use crate::{
    InspectError,
    format::{header_mask, page::PAGE_BYTES, row},
    header::{self, check},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const ROOT_PREFIX: [u8; 4] = [0, 1, 0, 0];
const ACE_ENGINE_NAME: &[u8; 16] = b"Standard ACE DB\0";
const ACE16_FORMAT_CODE: u8 = 5;
const CODE_PAGE_OFFSET: usize = 60;
const CODE_PAGE: usize = 1252;
const SORT_ORDER_OFFSET: usize = 110;

pub(super) fn build(
    request: &Request<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, CreateDatabaseError> {
    let extra = usize::from(request.primary.is_some());
    if profile::PAGE_COUNT + extra > profile::MAP_BITMAP_BYTES * u8::BITS as usize {
        return Err(CreateDatabaseError::PageCapacity);
    }
    let image_bytes = profile::IMAGE_BYTES + extra * PAGE_BYTES;
    let index_root = profile::PAGE_COUNT;
    let index_slot = profile::MAP_ROWS;
    let mut bytes = initialize(image_bytes, budget, cancelled)?;
    {
        let mut maps = DataPage::new(page(&mut bytes, profile::MAP_PAGE)?, 0)?;
        for slot in 0..profile::MAP_ROWS + extra {
            let member = if slot == index_slot {
                Some(index_root)
            } else {
                match slot {
                    profile::CATALOG_MAP_SLOT | profile::CATALOG_CANDIDATE_SLOT => {
                        Some(profile::CATALOG_DATA)
                    }
                    profile::USER_MAP_SLOT => Some(profile::USER_DATA),
                    profile::USER_CANDIDATE_SLOT
                        if request.free_bytes >= request.minimum_row_bytes + size_of::<u16>() =>
                    {
                        Some(profile::USER_DATA)
                    }
                    _ => None,
                }
            };
            record::inline_map(
                maps.append(profile::MAP_RECORD_BYTES, budget, cancelled)?,
                member,
            )?;
        }
        maps.finish()?;
    }

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
        (
            profile::USER_DEFINITION,
            request.fields(),
            request.rows.count(),
            profile::USER_MAP_SLOT,
        ),
    ] {
        check(budget, cancelled)?;
        if number == profile::USER_DEFINITION
            && let Some(primary) = request.primary.as_ref()
        {
            definition::write_role_with_primary(
                page(&mut bytes, number)?,
                fields,
                rows,
                definition::Role {
                    user: true,
                    catalog: false,
                    owned: row::reference(profile::MAP_PAGE as u32, profile::USER_MAP_SLOT as u8),
                    candidate: row::reference(
                        profile::MAP_PAGE as u32,
                        profile::USER_CANDIDATE_SLOT as u8,
                    ),
                },
                Some(definition::PrimaryRole {
                    prepared: primary,
                    root: index_root,
                    map: row::reference(profile::MAP_PAGE as u32, index_slot as u8),
                }),
                budget,
                cancelled,
            )?;
            continue;
        }
        definition::write(
            page(&mut bytes, number)?,
            fields,
            rows,
            slot,
            budget,
            cancelled,
        )?;
    }
    {
        let mut catalog = DataPage::new(
            page(&mut bytes, profile::CATALOG_DATA)?,
            profile::CATALOG_DEFINITION,
        )?;
        let user = profile::CatalogObject {
            id: profile::USER_DEFINITION as u32,
            parent: profile::TABLES_ID,
            name: request.catalog_name,
            system: false,
        };
        for object in systems() {
            record::catalog(
                catalog.append(record::catalog_length(object.name), budget, cancelled)?,
                &object,
                budget,
                cancelled,
            )?;
        }
        let properties = request.properties.as_ref();
        let length = record::catalog_length_with_properties(user.name, properties)?;
        record::catalog_with_properties(
            catalog.append(length, budget, cancelled)?,
            &user,
            properties,
            budget,
            cancelled,
        )?;
        catalog.finish()?;
    }
    {
        let mut users = DataPage::new(
            page(&mut bytes, profile::USER_DATA)?,
            profile::USER_DEFINITION,
        )?;
        for row in 0..request.rows.count() {
            record::user(
                users.append(request.row_length(row), budget, cancelled)?,
                request,
                row,
                budget,
                cancelled,
            )?;
        }
        users.finish()?;
    }
    if let Some(primary) = request.primary.as_ref() {
        primary.write_leaf(
            page(&mut bytes, index_root)?,
            profile::USER_DEFINITION,
            profile::USER_DATA,
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(bytes)
}

pub(super) fn page(bytes: &mut [u8], number: usize) -> Result<&mut [u8], CreateDatabaseError> {
    let start = number
        .checked_mul(PAGE_BYTES)
        .ok_or(CreateDatabaseError::Layout)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(CreateDatabaseError::Layout)?;
    bytes.get_mut(start..end).ok_or(CreateDatabaseError::Layout)
}

pub(super) fn initialize(
    image_bytes: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, CreateDatabaseError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(image_bytes as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(image_bytes)
        .map_err(|_| InspectError::Allocation)?;
    while bytes.len() < image_bytes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        bytes.resize(bytes.len() + PAGE_BYTES, 0);
    }

    let root = page(&mut bytes, 0)?;
    root[..ROOT_PREFIX.len()].copy_from_slice(&ROOT_PREFIX);
    root[ROOT_PREFIX.len()..header::FORMAT_CODE_OFFSET].copy_from_slice(ACE_ENGINE_NAME);
    root[header::FORMAT_CODE_OFFSET] = ACE16_FORMAT_CODE;
    definition::put16(root, CODE_PAGE_OFFSET, CODE_PAGE)?;
    root[SORT_ORDER_OFFSET..SORT_ORDER_OFFSET + profile::GENERAL_SORT.len()]
        .copy_from_slice(&profile::GENERAL_SORT);
    // Clear subversion/key/password/date and unmodeled header fields are
    // explicit authored zero choices. They are not declared reserved bytes,
    // imported identity seeds or evidence of native Access creation behavior.
    budget
        .charge(Resource::Steps, header_mask::PREFIX_END as u64)
        .map_err(InspectError::from)?;
    header_mask::apply(root)?;

    {
        let mut global = DataPage::new(
            page(&mut bytes, profile::GLOBAL_PAGE)?,
            profile::GLOBAL_PAGE,
        )?;
        record::inline_map(
            global.append(profile::MAP_RECORD_BYTES, budget, cancelled)?,
            None,
        )?;
        global.finish()?;
    }
    Ok(bytes)
}

pub(super) fn systems() -> [profile::CatalogObject<'static>; profile::SYSTEM_OBJECT_COUNT] {
    [
        (profile::ROOT_ID, 0, "DatabaseRoot"),
        (profile::TABLES_ID, profile::ROOT_ID, "Tables"),
        (profile::DATABASES_ID, profile::ROOT_ID, "Databases"),
        (profile::RELATIONSHIPS_ID, profile::ROOT_ID, "Relationships"),
        (
            profile::CATALOG_DEFINITION as u32,
            profile::TABLES_ID,
            "MSysObjects",
        ),
        (
            profile::RELATIONSHIPS_DEFINITION as u32,
            profile::TABLES_ID,
            "MSysRelationships",
        ),
        (
            profile::ACES_DEFINITION as u32,
            profile::TABLES_ID,
            "MSysACEs",
        ),
    ]
    .map(|(id, parent, name)| profile::CatalogObject {
        id,
        parent,
        name: Plan::ascii_literal(name),
        system: true,
    })
}
