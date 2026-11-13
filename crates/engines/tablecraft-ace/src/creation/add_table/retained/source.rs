//! Retained append placement consumes a complete table-owned physical inventory.
use super::super::{
    AddTableError, AddTableSourceCapabilities,
    geometry::{PrimaryRole, Role},
};
use crate::format::data_append::Page;
use crate::{
    FileFormat, FirstTableDefinition, InspectError, NativeImage, NativeTableStorage,
    compare_general_text,
    creation::{
        input::{Columns, Rows},
        record, request, stored_text,
    },
    format::page::PAGE_BYTES,
    header::check,
    limits::MAX_READ_FILE_BYTES,
    table::table_append::{
        Inventory,
        allocation::{Allocation, AllocationError},
    },
};
use std::cmp::Ordering;
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAP_RECORD_BYTES: usize = 13;
const DIRECTORY_HEADER_BYTES: usize = 14;
const SLOT_BYTES: usize = size_of::<u16>();
const MAX_SLOTS: usize = u8::MAX as usize + 1;

pub(super) struct Source<'image> {
    pub inventory: Inventory<'image>,
    pub allocation: Allocation<'image>,
    pub catalog: Page,
    pub catalog_page: u32,
    pub maps: Page,
    pub map_page: u32,
    pub added: Role,
    pub after_bytes: usize,
}

/// A stored database-property role selects the dialect before any source proof.
/// Missing, enabled or malformed OFF settings never trigger a legacy retry.
pub(in crate::creation::add_table) fn selected(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, AddTableError> {
    check(budget, cancelled)?;
    let header = image.catalog().header();
    if header.format() != FileFormat::Ace2016 || header.subversion() != 0 || header.encoded_pages()
    {
        return Ok(false);
    }
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, object.name().as_str().len() as u64)
            .map_err(InspectError::from)?;
        if object.name().as_str().eq_ignore_ascii_case("MSysDb")
            && origin.is_system()
            && origin.type_code() == 2
            && origin.table_storage() != Some(NativeTableStorage::Local)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn page(image: &NativeImage, number: u32) -> Result<&[u8], AddTableError> {
    let start = usize::try_from(number)
        .ok()
        .and_then(|value| value.checked_mul(PAGE_BYTES))
        .ok_or(AddTableError::Source)?;
    image
        .as_bytes()
        .get(start..start + PAGE_BYTES)
        .ok_or(AddTableError::Source)
}

fn fits(page: &Page, records: usize, payload: usize) -> bool {
    let Some(count) = page
        .count
        .checked_add(records)
        .filter(|count| *count <= MAX_SLOTS)
    else {
        return false;
    };
    let Some(required) = count
        .checked_mul(SLOT_BYTES)
        .and_then(|bytes| bytes.checked_add(DIRECTORY_HEADER_BYTES))
        .and_then(|bytes| bytes.checked_add(payload))
    else {
        return false;
    };
    page.start >= required
}

fn choose(
    image: &NativeImage,
    hosts: &[u32],
    owner: u32,
    records: usize,
    payload: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(u32, Page), AddTableError> {
    for &host in hosts {
        check(budget, cancelled)?;
        let captured = Page::capture_retained(page(image, host)?, owner, budget, cancelled)?;
        if fits(&captured, records, payload) {
            return Ok((host, captured));
        }
    }
    Err(AddTableError::Capacity)
}

fn require_name(
    image: &NativeImage,
    inventory: &Inventory<'_>,
    proposed: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        if origin.parent_id() == inventory.tables_parent()
            && compare_general_text(
                inventory.catalog_name_order(),
                proposed,
                object.name().as_str(),
                budget,
                cancelled,
            )? == Ordering::Equal
        {
            return Err(AddTableError::DuplicateName);
        }
    }
    Ok(())
}

fn after_bytes(before_pages: u32, keyed: bool) -> Result<usize, AddTableError> {
    let pages = u64::from(before_pages)
        .checked_add(2 + u64::from(keyed))
        .ok_or(AddTableError::Capacity)?;
    let length = pages
        .checked_mul(PAGE_BYTES as u64)
        .filter(|length| *length <= MAX_READ_FILE_BYTES)
        .ok_or(AddTableError::Capacity)?;
    usize::try_from(length).map_err(|_| AddTableError::Capacity)
}

pub(super) fn capture<'image>(
    image: &'image NativeImage,
    definition: &FirstTableDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source<'image>, AddTableError> {
    if !selected(image, budget, cancelled)? {
        return Err(AddTableError::Source);
    }
    image.require_retained_capture()?;
    let inventory = Inventory::capture(image, budget, cancelled).map_err(inventory_error)?;
    require_name(image, &inventory, definition.table(), budget, cancelled)?;
    let columns = definition.borrowed_columns(budget, cancelled)?;
    let request = request::validate(
        definition.table(),
        Columns::Values(&columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?;
    let length =
        record::catalog_length_with_properties(request.catalog_name, request.properties.as_ref())?;
    let keyed = definition.primary_position().is_some();
    let added_pages = 2 + usize::from(keyed);
    let (catalog_page, catalog) = choose(
        image,
        inventory.catalog_pages(),
        inventory.catalog_definition(),
        1,
        length,
        budget,
        cancelled,
    )?;
    let (map_page, maps) = choose(
        image,
        inventory.map_pages(),
        0,
        added_pages,
        added_pages * MAP_RECORD_BYTES,
        budget,
        cancelled,
    )?;
    let definition_page =
        usize::try_from(inventory.before_pages()).map_err(|_| AddTableError::Capacity)?;
    let after_bytes = after_bytes(inventory.before_pages(), keyed)?;
    let after_pages =
        u32::try_from(after_bytes / PAGE_BYTES).map_err(|_| AddTableError::Capacity)?;
    let allocation = inventory
        .plan_allocation(after_pages, budget, cancelled)
        .map_err(allocation_error)?;
    let added = Role {
        definition: definition_page,
        data: definition_page
            .checked_add(1)
            .ok_or(AddTableError::Capacity)?,
        primary: keyed.then(|| PrimaryRole {
            root: definition_page + 2,
            map: maps.count + 2,
        }),
        owned: maps.count,
        candidate: maps.count + 1,
        catalog: catalog.count,
    };
    Ok(Source {
        inventory,
        allocation,
        catalog,
        catalog_page,
        maps,
        map_page,
        added,
        after_bytes,
    })
}

pub(super) fn capabilities(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AddTableSourceCapabilities, AddTableError> {
    image.require_retained_capture()?;
    let inventory = Inventory::capture(image, budget, cancelled).map_err(inventory_error)?;
    let minimum_catalog = record::catalog_length(stored_text::Plan::ascii_literal("x"));
    match choose(
        image,
        inventory.catalog_pages(),
        inventory.catalog_definition(),
        1,
        minimum_catalog,
        budget,
        cancelled,
    ) {
        Ok(_) => {}
        Err(AddTableError::Capacity) => return Ok(AddTableSourceCapabilities::default()),
        Err(error) => return Err(error),
    }
    let mut capabilities = AddTableSourceCapabilities::default();
    for keyed in [false, true] {
        let count = 2 + usize::from(keyed);
        let available = after_bytes(inventory.before_pages(), keyed).is_ok()
            && match choose(
                image,
                inventory.map_pages(),
                0,
                count,
                count * MAP_RECORD_BYTES,
                budget,
                cancelled,
            ) {
                Ok(_) => true,
                Err(AddTableError::Capacity) => false,
                Err(error) => return Err(error),
            };
        let available = if available {
            let length = after_bytes(inventory.before_pages(), keyed)?;
            let pages = u32::try_from(length / PAGE_BYTES).map_err(|_| AddTableError::Capacity)?;
            match inventory.plan_allocation(pages, budget, cancelled) {
                Ok(_) => true,
                Err(AllocationError::Capacity) => false,
                Err(error) => return Err(allocation_error(error)),
            }
        } else {
            false
        };
        if keyed {
            capabilities.keyed = available;
        } else {
            capabilities.keyless = available;
        }
    }
    Ok(capabilities)
}

pub(super) fn inventory_error(error: crate::EditError) -> AddTableError {
    match error {
        crate::EditError::Inspect(error) => AddTableError::Inspect(error),
        crate::EditError::Preservation => AddTableError::Preservation,
        _ => AddTableError::Source,
    }
}

pub(in crate::creation::add_table) fn allocation_error(error: AllocationError) -> AddTableError {
    match error {
        AllocationError::Inspect(error) => AddTableError::Inspect(error),
        AllocationError::Capacity => AddTableError::Capacity,
        AllocationError::Source => AddTableError::Source,
    }
}

pub(in crate::creation::add_table) fn allocation_preservation_error(
    error: AllocationError,
) -> AddTableError {
    match error {
        AllocationError::Source => AddTableError::Preservation,
        error => allocation_error(error),
    }
}
