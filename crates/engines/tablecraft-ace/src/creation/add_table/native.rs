//! Native-count append reuses actual packed or retained physical ownership.
mod maps;

use super::{
    AddTableError, AddTablePlan, AddTableSourceCapabilities, AddTableSourceProfile, retained,
};
use crate::{
    FirstTableDefinition, InspectError, NativeImage, NativeImageSource, compare_general_text,
    creation::{
        catalog_page::Page,
        definition,
        input::{Columns, Rows},
        native::{self, MapReferences, topology::Topology},
        profile,
        record::{self, DataPage},
        request,
    },
    format::{
        bytes::{u16_at, u32_at},
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        row::{OFFSET_MASK, reference, reference_page, reference_slot, slot_offset},
    },
    header::check,
    table::table_append::Inventory,
};
use maps::Map;
use std::{cmp::Ordering, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

const EXTERNAL_DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const MAP_BYTES: usize = 13;
const MAX_SLOTS: usize = u8::MAX as usize + 1;

fn page(bytes: &[u8], number: u32) -> Result<&[u8], AddTableError> {
    let start = number as usize * PAGE_BYTES;
    bytes
        .get(start..start + PAGE_BYTES)
        .ok_or(AddTableError::Source)
}

fn page_mut(bytes: &mut [u8], number: u32) -> Result<&mut [u8], AddTableError> {
    let start = number as usize * PAGE_BYTES;
    bytes
        .get_mut(start..start + PAGE_BYTES)
        .ok_or(AddTableError::Preservation)
}

fn map_error(error: crate::EditError) -> AddTableError {
    match error {
        crate::EditError::Inspect(error) => AddTableError::Inspect(error),
        _ => AddTableError::Source,
    }
}

fn replacement(host: u32, slot: usize, maps: &[Map]) -> Option<&[u8]> {
    maps.iter()
        .find(|map| {
            reference_page(map.reference) == host
                && usize::from(reference_slot(map.reference)) == slot
        })
        .map(|map| map.bytes.as_slice())
}

fn map_capacity(
    host: u32,
    captured: &Page,
    maps: &[Map],
    append: usize,
) -> Result<bool, AddTableError> {
    let count = captured
        .count
        .checked_add(append)
        .ok_or(AddTableError::Capacity)?;
    if count > MAX_SLOTS {
        return Ok(false);
    }
    let mut used = slot_offset(count)
        .checked_add(append * MAP_BYTES)
        .ok_or(AddTableError::Capacity)?;
    for (slot, range) in captured.records.iter().enumerate() {
        used = used
            .checked_add(replacement(host, slot, maps).map_or(range.len(), <[u8]>::len))
            .ok_or(AddTableError::Capacity)?;
    }
    Ok(used <= PAGE_BYTES)
}

fn map_record(destination: &mut [u8], member: Option<usize>) -> Result<(), AddTableError> {
    destination.fill(0);
    let Some(member) = member else {
        return Ok(());
    };
    let base = member / 64 * 64;
    definition::put32(destination, 1, base as u32)?;
    let bit = member - base;
    destination[5 + bit / 8] = 1 << (bit % 8);
    Ok(())
}

fn capture_inventory<'image>(
    image: &'image NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Inventory<'image>, bool), AddTableError> {
    let is_retained = retained::selected(image, budget, cancelled)?;
    let inventory = if is_retained {
        Inventory::capture_represented(image, budget, cancelled)
    } else {
        Inventory::capture_packed(image, budget, cancelled)
    }
    .map_err(map_error)?;
    Ok((inventory, is_retained))
}

/// Check the original plan's actual catalog hosts for the complete inline row.
/// The caller has independently proved its frozen source/map eligibility.
pub(super) fn original_catalog_fits(
    image: &NativeImage,
    length: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, AddTableError> {
    if !retained::selected(image, budget, cancelled)? {
        // Frozen packed eligibility was positively proved by the caller. Do not
        // add a native census requirement to an otherwise valid Original plan.
        let captured = Page::capture(
            page(image.as_bytes(), super::source::CATALOG_PAGE as u32)?,
            profile::CATALOG_DEFINITION as u32,
            budget,
            cancelled,
        )?;
        return Ok(captured.count < MAX_SLOTS
            && captured.start >= slot_offset(captured.count + 1) + length);
    }
    let inventory = Inventory::capture(image, budget, cancelled).map_err(map_error)?;
    for &host in inventory.catalog_pages() {
        check(budget, cancelled)?;
        let captured = Page::capture_retained(
            page(image.as_bytes(), host)?,
            inventory.catalog_definition(),
            budget,
            cancelled,
        )?;
        if captured.count < MAX_SLOTS && captured.start >= slot_offset(captured.count + 1) + length
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Advertise a minimal named schema from actual catalog/map/global capacity.
/// This is complete source qualification, never a draft or an apply capability.
pub(super) fn capabilities(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AddTableSourceCapabilities, AddTableError> {
    let (inventory, _) = capture_inventory(image, budget, cancelled)?;
    let length = record::catalog_length(crate::creation::stored_text::Plan::ascii_literal("T"));
    let mut fits_catalog = false;
    for &host in inventory.catalog_pages() {
        check(budget, cancelled)?;
        let captured = Page::capture_retained(
            page(image.as_bytes(), host)?,
            inventory.catalog_definition(),
            budget,
            cancelled,
        )?;
        if captured.count < MAX_SLOTS && captured.start >= slot_offset(captured.count + 1) + length
        {
            fits_catalog = true;
            break;
        }
    }
    if !fits_catalog {
        return Ok(AddTableSourceCapabilities::default());
    }
    let mut capabilities = AddTableSourceCapabilities::default();
    for keyed in [false, true] {
        check(budget, cancelled)?;
        let added = 2 + usize::from(keyed);
        let mut fits_maps = false;
        for &host in inventory.map_pages() {
            let captured =
                Page::capture_retained(page(image.as_bytes(), host)?, 0, budget, cancelled)?;
            if map_capacity(host, &captured, &[], added)? {
                fits_maps = true;
                break;
            }
        }
        let pages = inventory.before_pages().checked_add(added as u32);
        let fits_native = pages.is_some_and(|pages| {
            u64::from(pages) * PAGE_BYTES as u64 <= crate::limits::MAX_READ_FILE_BYTES
        });
        let fits_allocation = if fits_maps && fits_native {
            match inventory.plan_allocation(
                pages.ok_or(AddTableError::Capacity)?,
                budget,
                cancelled,
            ) {
                Ok(_) => true,
                Err(crate::table::table_append::allocation::AllocationError::Capacity) => false,
                Err(error) => return Err(super::retained::source::allocation_error(error)),
            }
        } else {
            false
        };
        if keyed {
            capabilities.keyed = fits_allocation;
        } else {
            capabilities.keyless = fits_allocation;
        }
    }
    Ok(capabilities)
}

pub(super) fn plan(
    image: NativeImage,
    declaration: FirstTableDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AddTablePlan, AddTableError> {
    check(budget, cancelled)?;
    if !declaration.is_native() {
        return Err(crate::CreateDatabaseError::Declaration.into());
    }
    let (inventory, is_retained) = capture_inventory(&image, budget, cancelled)?;
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
                declaration.table(),
                object.name().as_str(),
                budget,
                cancelled,
            )? == Ordering::Equal
        {
            return Err(AddTableError::DuplicateName);
        }
    }
    let columns = declaration.borrowed_columns(budget, cancelled)?;
    let request = request::validate_native(
        declaration.table(),
        Columns::Values(&columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?;
    let logical_length =
        definition::length_with_primary(request.fields(), false, request.primary.as_ref())?;
    let body_length = request
        .properties
        .as_ref()
        .map_or(0, |body| body.body_length());
    let mut topology = Topology::capture(
        inventory.before_pages() as usize,
        logical_length,
        body_length,
        request.primary.is_some(),
    )?;
    // Column-map storage is planned from its actual old representation. Existing
    // indirect bitmap identities remain live; only genuinely missing blocks are
    // allocated after the new property chain.
    topology.end = topology.properties.end;
    topology.bitmaps = topology.end..topology.end;
    let long_values = crate::check_table_long_values(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "MSysObjects",
        budget,
        cancelled,
    )?;
    let references = long_values
        .columns()
        .iter()
        .find(|column| column.column_number() == 14)
        .and_then(|column| column.maps())
        .ok_or(AddTableError::Source)?;
    if references.owned() == references.candidate() {
        return Err(AddTableError::Source);
    }
    let mut next_page = topology.end;
    let owned = Map::capture(
        &image,
        references.owned(),
        (topology.properties.start..topology.properties.end).map(|number| number as u32),
        &mut next_page,
        budget,
        cancelled,
    )?;
    let candidate = Map::capture(
        &image,
        references.candidate(),
        topology.candidate.into_iter().map(|number| number as u32),
        &mut next_page,
        budget,
        cancelled,
    )?;
    let maps = [owned, candidate];
    for owned in &maps[0].bitmaps {
        for candidate in &maps[1].bitmaps {
            check(budget, cancelled)?;
            if owned.number == candidate.number {
                return Err(AddTableError::Source);
            }
        }
    }
    topology.end = next_page;
    if (topology.end as u64) * PAGE_BYTES as u64 > crate::limits::MAX_READ_FILE_BYTES {
        return Err(AddTableError::Capacity);
    }
    let allocation = inventory
        .plan_allocation(
            u32::try_from(topology.end).map_err(|_| AddTableError::Capacity)?,
            budget,
            cancelled,
        )
        .map_err(super::retained::source::allocation_error)?;
    let catalog_length = record::catalog_length(request.catalog_name)
        + if body_length == 0 {
            0
        } else {
            EXTERNAL_DESCRIPTOR_BYTES
        };
    let mut catalog_host = None;
    for &host in inventory.catalog_pages() {
        let captured = Page::capture_retained(
            page(image.as_bytes(), host)?,
            inventory.catalog_definition(),
            budget,
            cancelled,
        )?;
        if captured.count < MAX_SLOTS
            && captured.start >= slot_offset(captured.count + 1) + catalog_length
        {
            catalog_host = Some((host, captured));
            break;
        }
    }
    let (catalog_host, catalog_before) = catalog_host.ok_or(AddTableError::Capacity)?;
    let additional_maps = 2 + usize::from(topology.index.is_some());
    let mut map_host = None;
    for &host in inventory.map_pages() {
        let captured = Page::capture_retained(page(image.as_bytes(), host)?, 0, budget, cancelled)?;
        if map_capacity(host, &captured, &maps, additional_maps)? {
            map_host = Some((host, captured.count));
            break;
        }
    }
    let (map_host, map_count) = map_host.ok_or(AddTableError::Capacity)?;
    for map in &maps {
        if !inventory
            .map_pages()
            .contains(&reference_page(map.reference))
        {
            return Err(AddTableError::Source);
        }
    }
    let length = topology
        .end
        .checked_mul(PAGE_BYTES)
        .ok_or(AddTableError::Capacity)?;
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in image.as_bytes().chunks(PAGE_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    while after.len() < length {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        after.resize(after.len() + PAGE_BYTES, 0);
    }
    allocation
        .apply(&mut after, budget, cancelled)
        .map_err(super::retained::source::allocation_preservation_error)?;
    native::write_table(
        &mut after,
        &request,
        &topology,
        MapReferences {
            owned: reference(map_host, map_count as u8),
            candidate: reference(map_host, (map_count + 1) as u8),
            index: topology
                .index
                .map(|_| reference(map_host, (map_count + 2) as u8)),
        },
        budget,
        cancelled,
    )?;
    for &host in inventory.map_pages() {
        let append = if host == map_host { additional_maps } else { 0 };
        if append == 0 && !maps.iter().any(|map| reference_page(map.reference) == host) {
            continue;
        }
        let original = page(image.as_bytes(), host)?;
        let captured = Page::capture_retained(original, 0, budget, cancelled)?;
        if !map_capacity(host, &captured, &maps, append)? {
            return Err(AddTableError::Capacity);
        }
        let target = page_mut(&mut after, host)?;
        let mut writer = DataPage::new(target, 0)?;
        for (slot, range) in captured.records.iter().enumerate() {
            let record =
                replacement(host, slot, &maps).unwrap_or(&original[range.start..range.end]);
            writer
                .append(record.len(), budget, cancelled)?
                .copy_from_slice(record);
        }
        if append != 0 {
            for member in [
                Some(topology.data),
                request.minimum_row_fits().then_some(topology.data),
                topology.index,
            ]
            .into_iter()
            .take(additional_maps)
            {
                map_record(writer.append(MAP_BYTES, budget, cancelled)?, member)?;
            }
        }
        writer.finish()?;
        // Map records keep their physical slot identities and original flags.
        for slot in 0..captured.count {
            let old = u16_at(original, slot_offset(slot))?;
            let new = u16_at(target, slot_offset(slot))?;
            definition::put16(
                target,
                slot_offset(slot),
                usize::from(new | (old & !OFFSET_MASK)),
            )?;
        }
    }
    for map in &maps {
        for bitmap in &map.bitmaps {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            page_mut(&mut after, bitmap.number)?.copy_from_slice(&bitmap.bytes);
        }
    }
    let mut catalog = Page::capture_retained(
        page(&after, catalog_host)?,
        inventory.catalog_definition(),
        budget,
        cancelled,
    )?;
    Page::charge_append(catalog_length, budget, cancelled)?;
    let span = catalog.append(page_mut(&mut after, catalog_host)?, catalog_length)?;
    let object = profile::CatalogObject {
        id: topology.definition.start as u32,
        parent: inventory.tables_parent() as u32,
        name: request.catalog_name,
        system: false,
    };
    if body_length == 0 {
        record::catalog(
            &mut page_mut(&mut after, catalog_host)?[span],
            &object,
            budget,
            cancelled,
        )?;
    } else {
        record::catalog_with_linked_properties(
            &mut page_mut(&mut after, catalog_host)?[span],
            &object,
            reference(topology.properties.start as u32, 0),
            body_length,
            budget,
            cancelled,
        )?;
    }
    let live = inventory
        .catalog_live_rows()
        .checked_add(1)
        .ok_or(AddTableError::Capacity)?;
    definition::put32(
        page_mut(&mut after, inventory.catalog_definition())?,
        RECORD_COUNT_OFFSET,
        live,
    )?;
    allocation
        .verify_after(&after, budget, cancelled)
        .map_err(super::retained::source::allocation_preservation_error)?;
    verify_original(
        &image,
        &after,
        Preservation {
            inventory: &inventory,
            catalog_host,
            catalog_before: &catalog_before,
            maps: &maps,
            new_map_host: map_host,
            added_maps: additional_maps,
            allocation: allocation.allowed_before_ranges(),
        },
        budget,
        cancelled,
    )?;
    drop(allocation);
    drop(inventory);
    let after = NativeImageSource::from_vec(after, budget, cancelled)?;
    // NO_CLONE: The independently inspected AFTER and returned plan share the
    // same complete immutable allocation. Neither owner can borrow the other
    // across native capture and later journal preparation.
    let captured = NativeImage::capture_shared_retained(
        after.clone(),
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    if is_retained {
        Inventory::capture_represented(&captured, budget, cancelled)
    } else {
        Inventory::capture_packed(&captured, budget, cancelled)
    }
    .map_err(map_error)?;
    let checked = crate::check_table(
        &mut captured.reader(),
        captured.catalog().catalog().scope(),
        declaration.table(),
        budget,
        cancelled,
    )?;
    let schema = checked.schema();
    if schema.definition_page() as usize != topology.definition.start || schema.declared_rows() != 0
    {
        return Err(AddTableError::Preservation);
    }
    super::audit::columns::verify(
        schema,
        &columns,
        topology.index.is_some(),
        budget,
        cancelled,
    )?;
    super::audit::properties::verify(&captured, schema, &columns, budget, cancelled)?;
    drop(request);
    drop(columns);
    Ok(AddTablePlan {
        before: image,
        after,
        definition: declaration,
        profile: if is_retained {
            AddTableSourceProfile::Retained
        } else {
            AddTableSourceProfile::Mixed
        },
    })
}

struct Preservation<'facts, 'image> {
    inventory: &'facts Inventory<'image>,
    catalog_host: u32,
    catalog_before: &'facts Page,
    maps: &'facts [Map],
    new_map_host: u32,
    added_maps: usize,
    allocation: &'facts [Range<usize>],
}

fn verify_original(
    before: &NativeImage,
    after: &[u8],
    facts: Preservation<'_, '_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    let Preservation {
        inventory,
        catalog_host,
        catalog_before,
        maps,
        new_map_host,
        added_maps,
        allocation,
    } = facts;
    if u32_at(
        page(after, inventory.catalog_definition())?,
        RECORD_COUNT_OFFSET,
    )? != inventory
        .catalog_live_rows()
        .checked_add(1)
        .ok_or(AddTableError::Preservation)?
    {
        return Err(AddTableError::Preservation);
    }
    let catalog_after = Page::capture_retained(
        page(after, catalog_host)?,
        inventory.catalog_definition(),
        budget,
        cancelled,
    )?;
    if catalog_after.count != catalog_before.count + 1 {
        return Err(AddTableError::Preservation);
    }
    for number in 0..inventory.before_pages() {
        check(budget, cancelled)?;
        let old = page(before.as_bytes(), number)?;
        let new = page(after, number)?;
        let map_host = inventory.map_pages().contains(&number)
            && (number == new_map_host
                || maps
                    .iter()
                    .any(|map| reference_page(map.reference) == number));
        if map_host {
            let old_page = Page::capture_retained(old, 0, budget, cancelled)?;
            let new_page = Page::capture_retained(new, 0, budget, cancelled)?;
            if new_page.count
                != old_page.count
                    + if number == new_map_host {
                        added_maps
                    } else {
                        0
                    }
            {
                return Err(AddTableError::Preservation);
            }
            for (slot, (old_range, new_range)) in
                old_page.records.iter().zip(&new_page.records).enumerate()
            {
                check(budget, cancelled)?;
                let expected =
                    replacement(number, slot, maps).unwrap_or(&old[old_range.start..old_range.end]);
                if &new[new_range.start..new_range.end] != expected
                    || u16_at(old, slot_offset(slot))? & !OFFSET_MASK
                        != u16_at(new, slot_offset(slot))? & !OFFSET_MASK
                {
                    return Err(AddTableError::Preservation);
                }
            }
            // Existing opaque free-gap bytes and all header fields outside the
            // measured free/count/directory changes remain byte-exact.
            for offset in 0..new_page.start {
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                let changed =
                    (2..4).contains(&offset) || (12..slot_offset(new_page.count)).contains(&offset);
                if !changed && old[offset] != new[offset] {
                    return Err(AddTableError::Preservation);
                }
            }
            continue;
        }
        let bitmap = maps
            .iter()
            .flat_map(|map| &map.bitmaps)
            .find(|bitmap| bitmap.number == number);
        if let Some(bitmap) = bitmap {
            if new != bitmap.bytes {
                return Err(AddTableError::Preservation);
            }
            continue;
        }
        for offset in 0..PAGE_BYTES {
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            let absolute = number as usize * PAGE_BYTES + offset;
            let permitted = allocation.iter().any(|range| range.contains(&absolute))
                || (number == inventory.catalog_definition()
                    && (RECORD_COUNT_OFFSET..RECORD_COUNT_OFFSET + size_of::<u32>())
                        .contains(&offset))
                || (number == catalog_host
                    && ((2..4).contains(&offset)
                        || (12..14).contains(&offset)
                        || (slot_offset(catalog_before.count)..slot_offset(catalog_after.count))
                            .contains(&offset)
                        || (catalog_after.start..catalog_before.start).contains(&offset)));
            if !permitted && old[offset] != new[offset] {
                return Err(AddTableError::Preservation);
            }
        }
    }
    Ok(())
}
