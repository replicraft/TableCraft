//! Missing system storage is constructed only from its own complete source proof.
mod fields;
mod proof;

use super::{append, record, source::initialization as source};
use crate::{
    InspectError, NativeImage, NativeImageSource, QueryCreationDefinition, QueryCreationError,
    creation::{definition, profile::Fields, record::DataPage},
    format::{data_append::Page, page::PAGE_BYTES},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAP_RECORD_BYTES: usize = 13;
const MAP_BITMAP_START: usize = 1 + size_of::<u32>();
const MAP_WINDOW_PAGES: u32 = 8 * u8::BITS;

pub(super) fn build(
    image: &NativeImage,
    requested: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, QueryCreationError> {
    let source = source::capture(image, budget, cancelled)?;
    let system = record::query_storage_catalog(
        &source.catalog,
        source.parent,
        source.definition_page,
        requested.captured_at(),
        budget,
        cancelled,
    )?;
    let length = (source.data_page as usize + 1)
        .checked_mul(PAGE_BYTES)
        .ok_or(QueryCreationError::Source)?;
    budget.charge_allocation(length as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in image.as_bytes().chunks(PAGE_BYTES) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, chunk.len() as u64)?;
        bytes.extend_from_slice(chunk);
    }
    budget.charge(Resource::Steps, (length - bytes.len()) as u64)?;
    bytes.resize(length, 0);
    source
        .allocation
        .apply(&mut bytes, budget, cancelled)
        .map_err(source::allocation_error)?;

    let maps = source.map_references()?;
    definition::write_role_with_long_values(
        append::page_mut(&mut bytes, source.definition_page)?,
        Fields::new(&fields::FIELDS),
        0,
        definition::Role {
            user: false,
            catalog: false,
            owned: maps[0],
            candidate: maps[1],
            text_sort_order: Some(source.order),
        },
        &[definition::LongValueRole {
            column: 5,
            owned: maps[2],
            candidate: maps[3],
        }],
        budget,
        cancelled,
    )
    .map_err(source::creation_error)?;
    DataPage::new(
        append::page_mut(&mut bytes, source.data_page)?,
        source.definition_page as usize,
    )
    .and_then(DataPage::finish)
    .map_err(source::creation_error)?;
    append::append(
        &mut bytes,
        &source.catalog,
        source.catalog_page,
        std::iter::once(system.as_slice()),
        budget,
        cancelled,
    )?;
    drop(system);
    let page = append::page_mut(&mut bytes, source.map_page)?;
    let mut frame = Page::capture_retained(page, 0, budget, cancelled)?;
    for ordinal in 0..4 {
        Page::charge_append(MAP_RECORD_BYTES, budget, cancelled)?;
        let range = frame.append(page, MAP_RECORD_BYTES)?;
        if ordinal < 2 {
            let base = source.data_page / MAP_WINDOW_PAGES * MAP_WINDOW_PAGES;
            let bit = source.data_page - base;
            let row = &mut page[range];
            row[1..5].copy_from_slice(&base.to_le_bytes());
            row[MAP_BITMAP_START + bit as usize / u8::BITS as usize] = 1 << (bit % u8::BITS);
        }
    }
    let owner = NativeImageSource::from_vec(bytes, budget, cancelled)?;
    let after = NativeImage::capture_shared_retained(
        owner,
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    proof::verify(image, &after, requested, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(after)
}
