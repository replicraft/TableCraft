//! One paid image allocation followed by direct, bounded serialization.
use super::{
    CreateDatabaseError, definition,
    profile::{self, Fields},
    record::{self, DataPage},
    request::Request,
};
use crate::{
    InspectError,
    format::{header_mask, page::PAGE_BYTES},
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
    check(budget, cancelled)?;
    budget
        .charge_allocation(profile::IMAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(profile::IMAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    while bytes.len() < profile::IMAGE_BYTES {
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
    {
        let mut maps = DataPage::new(page(&mut bytes, profile::MAP_PAGE)?, 0)?;
        for slot in 0..profile::MAP_ROWS {
            let member = match slot {
                profile::CATALOG_MAP_SLOT | profile::CATALOG_CANDIDATE_SLOT => {
                    Some(profile::CATALOG_DATA)
                }
                profile::USER_MAP_SLOT => Some(profile::USER_DATA),
                profile::USER_CANDIDATE_SLOT
                    if request.free_bytes >= request.row_bytes + size_of::<u16>() =>
                {
                    Some(profile::USER_DATA)
                }
                _ => None,
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
            Fields::Known(&profile::CATALOG_FIELDS),
            profile::CATALOG_OBJECT_COUNT,
            profile::CATALOG_MAP_SLOT,
        ),
        (
            profile::RELATIONSHIPS_DEFINITION,
            Fields::Known(&profile::RELATIONSHIP_FIELDS),
            0,
            profile::RELATIONSHIPS_MAP_SLOT,
        ),
        (
            profile::ACES_DEFINITION,
            Fields::Known(&profile::ACE_FIELDS),
            0,
            profile::ACES_MAP_SLOT,
        ),
        (
            profile::USER_DEFINITION,
            Fields::User(request.columns),
            request.rows.len(),
            profile::USER_MAP_SLOT,
        ),
    ] {
        check(budget, cancelled)?;
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
        for (id, parent, name, system) in [
            (profile::ROOT_ID, 0, "DatabaseRoot", true),
            (profile::TABLES_ID, profile::ROOT_ID, "Tables", true),
            (profile::DATABASES_ID, profile::ROOT_ID, "Databases", true),
            (
                profile::RELATIONSHIPS_ID,
                profile::ROOT_ID,
                "Relationships",
                true,
            ),
            (
                profile::CATALOG_DEFINITION as u32,
                profile::TABLES_ID,
                "MSysObjects",
                true,
            ),
            (
                profile::RELATIONSHIPS_DEFINITION as u32,
                profile::TABLES_ID,
                "MSysRelationships",
                true,
            ),
            (
                profile::ACES_DEFINITION as u32,
                profile::TABLES_ID,
                "MSysACEs",
                true,
            ),
            (
                profile::USER_DEFINITION as u32,
                profile::TABLES_ID,
                request.table,
                false,
            ),
        ] {
            let object = profile::CatalogObject {
                id,
                parent,
                name,
                system,
            };
            record::catalog(
                catalog.append(record::catalog_length(name), budget, cancelled)?,
                &object,
                budget,
                cancelled,
            )?;
        }
        catalog.finish()?;
    }
    {
        let mut users = DataPage::new(
            page(&mut bytes, profile::USER_DATA)?,
            profile::USER_DEFINITION,
        )?;
        for row in request.rows {
            record::user(users.append(request.row_bytes, budget, cancelled)?, row)?;
        }
        users.finish()?;
    }
    check(budget, cancelled)?;
    Ok(bytes)
}

fn page(bytes: &mut [u8], number: usize) -> Result<&mut [u8], CreateDatabaseError> {
    let start = number
        .checked_mul(PAGE_BYTES)
        .ok_or(CreateDatabaseError::Layout)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(CreateDatabaseError::Layout)?;
    bytes.get_mut(start..end).ok_or(CreateDatabaseError::Layout)
}
