//! Complete fresh index effect shared by the closed LONG and numeric candidates.
use super::{
    index_creation::{
        buffer,
        definition::{self, CreatedDescriptor},
    },
    index_population_audit as audit, index_population_maps as maps,
    index_population_source as source,
};
use crate::{
    EditError, InspectError, NativeImage, NativeImageSource, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    table::{
        indexes::mutation::bulk_numeric::Population, table_append::allocation::AllocationError,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Built {
    pub before: NativeImage,
    pub after: NativeImage,
    pub construction: source::Construction,
    pub policy: super::index_creation::IndexPolicy,
    pub scalar: tablecraft_schema::ScalarType,
    pub table: String,
    pub column: String,
    pub index: tablecraft_schema::SchemaName,
    pub native_table: i32,
    pub definition: u32,
    pub native: usize,
    pub column_id: u16,
    pub descending: bool,
    pub logical: u32,
    pub physical: u32,
    pub rows: usize,
    pub root: u32,
    pub pages: usize,
    pub height: u8,
}

pub(super) fn build<const WIRE_BYTES: usize>(
    image: NativeImage,
    table: &str,
    request: &super::index_creation::SecondaryIndexCreate<'_>,
    mode: source::Mode,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Built, EditError> {
    let original = source::capture_mode(&image, table, request, mode, budget, cancelled)?;
    let population = if original.is_unique() {
        Population::<WIRE_BYTES>::capture_unique_long(
            original.keys(budget, cancelled)?,
            original.native,
            original.descending,
            original.index_start,
            budget,
            cancelled,
        )?
    } else {
        Population::<WIRE_BYTES>::capture(
            original.keys(budget, cancelled)?,
            original.native,
            original.kind,
            original.descending,
            original.index_start,
            budget,
            cancelled,
        )?
    };

    let maps = maps::build(
        original.index_start,
        population.page_count(),
        budget,
        cancelled,
    )?;
    original.inventory.require_source(&image)?;
    let allocation = original
        .inventory
        .plan_allocation(maps.after_pages, budget, cancelled)
        .map_err(allocation_error)?;
    let descriptor = CreatedDescriptor {
        column: original.native,
        descending: original.descending,
        counter: population.distinct(),
        physical: original.layout.physical_count,
        logical: original.logical,
        usage: maps.usage,
        root: population.root(),
    };
    let definition = if original.is_unique() {
        definition::compose_unique_long(
            &image,
            &original.layout,
            &original.name,
            descriptor,
            budget,
            cancelled,
        )?
    } else {
        match original.construction {
            source::Construction::OriginalCount => definition::compose(
                &image,
                &original.layout,
                &original.name,
                descriptor,
                budget,
                cancelled,
            )?,
            source::Construction::Fresh => definition::compose_fresh(
                &image,
                &original.layout,
                &original.name,
                descriptor,
                budget,
                cancelled,
            )?,
        }
    };

    let length = usize::try_from(maps.after_pages)
        .map_err(|_| InspectError::StructureLimit)?
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let mut bytes = buffer::vector(length, budget, cancelled)?;
    for chunk in image
        .as_bytes()
        .chunks(crate::preservation::COPY_CHUNK_BYTES)
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        bytes.extend_from_slice(chunk);
    }
    let pages = population.build(original.layout.definition, budget, cancelled)?;
    for page in pages.into_iter().chain(maps.pages) {
        check(budget, cancelled)?;
        if page.number as usize * PAGE_BYTES != bytes.len() || page.bytes.len() != PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        bytes.extend_from_slice(&page.bytes);
    }
    if bytes.len() != length {
        return Err(EditError::Preservation);
    }
    allocation
        .apply(&mut bytes, budget, cancelled)
        .map_err(allocation_error)?;
    let start = original.layout.definition as usize * PAGE_BYTES;
    bytes[start..start + PAGE_BYTES].copy_from_slice(&definition);
    let scope = image.catalog().catalog().scope();
    let after = NativeImage::capture_shared_retained(
        NativeImageSource::from_vec(bytes, budget, cancelled)?,
        scope,
        budget,
        cancelled,
    )?;
    if original.is_unique() {
        audit::prove_unique(&image, &after, table, request, budget, cancelled)?;
    } else if original.is_numeric() {
        audit::prove_numeric(&image, &after, table, request, budget, cancelled)?;
    } else {
        match original.construction {
            source::Construction::OriginalCount => {
                audit::prove(&image, &after, table, request, budget, cancelled)?;
            }
            source::Construction::Fresh => {
                audit::prove_fresh(&image, &after, table, request, budget, cancelled)?;
            }
        }
    }
    let table = String::from_utf8(buffer::copy(table.as_bytes(), budget, cancelled)?)
        .map_err(|_| InspectError::Malformed)?;
    let column = String::from_utf8(buffer::copy(request.column.as_bytes(), budget, cancelled)?)
        .map_err(|_| InspectError::Malformed)?;
    let rows = original.content.records.len();
    let policy = if original.is_unique() {
        super::index_creation::IndexPolicy::UniqueLong
    } else {
        super::index_creation::IndexPolicy::Ordinary
    };
    let source::Original {
        inventory,
        scalar,
        construction,
        name: index,
        native_table,
        layout,
        native,
        column_id,
        descending,
        logical,
        ..
    } = original;
    // End the certificate's borrow before moving the caller-owned BEFORE image.
    drop(inventory);
    Ok(Built {
        before: image,
        after,
        construction,
        policy,
        scalar,
        table,
        column,
        index,
        native_table,
        definition: layout.definition,
        native,
        column_id,
        descending,
        logical,
        physical: layout.physical_count,
        rows,
        root: population.root(),
        pages: population.page_count(),
        height: population.height(),
    })
}

pub(super) fn allocation_error(error: AllocationError) -> EditError {
    match error {
        AllocationError::Inspect(error) => error.into(),
        AllocationError::Capacity => EditError::Unsupported(WriteBlocker::RowLayout),
        AllocationError::Source => EditError::Preservation,
    }
}
