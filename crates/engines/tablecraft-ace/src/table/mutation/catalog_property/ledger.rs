//! Image-bound observation of the column ledger and its separate live owners.
//! Complete mutation/source admission remains with each calling feature.
use super::{column, descriptor, maps};
use crate::{
    CatalogInspection, EditError, InspectError, NativeObjectOrigin,
    format::{
        page::PAGE_BYTES,
        reader::Reader,
        row::{reference, reference_page},
    },
    header::check,
    inspect_header, inspect_table_schema,
    table::{
        long_value::trace::{Chunk, Kind},
        mutation::owned_lval::maps::{EofAllocation, Maps},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::ObjectId;

struct Live<'a> {
    origin: &'a NativeObjectOrigin,
    descriptor: descriptor::Descriptor,
    kind: Kind,
    chunks: Vec<Chunk>,
    bytes: usize,
}

/// Borrows both the actual bytes and their captured catalog identity. It is not
/// an allocation plan or permission to mutate a role or a historical row.
pub(crate) struct Ledger<'a> {
    _bytes: &'a [u8],
    pages: Vec<u32>,
    live: Vec<Live<'a>>,
}

pub(crate) fn capture<'a>(
    bytes: &'a [u8],
    catalog: &'a CatalogInspection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ledger<'a>, EditError> {
    let header = inspect_header(&mut Cursor::new(bytes), budget, cancelled)?;
    if header != catalog.header() {
        return Err(InspectError::Malformed.into());
    }
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        catalog.catalog().scope(),
        "MSysObjects",
        budget,
        cancelled,
    )?;
    if usize::try_from(schema.declared_rows()).ok() != Some(catalog.origins().len()) {
        return Err(InspectError::Unsupported.into());
    }
    let selected = column(&schema)?;
    let mut input = Cursor::new(bytes);
    let mut reader = Reader {
        source: &mut input,
        header,
        budget,
        cancelled,
        reads: 0,
    };
    let references = maps::references(&mut reader, &schema, selected)?;
    // This observes all allocated column pages, including a keyless options
    // page9. The existing property writer separately restricts pages10/11.
    let owned = Maps::capture_from_reader(
        &mut reader,
        &schema,
        references,
        EofAllocation::RequireAllocated,
    )?;
    reader
        .budget
        .charge_allocation(catalog.origins().len() as u64, size_of::<Live<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut live: Vec<Live<'_>> = Vec::new();
    live.try_reserve_exact(catalog.origins().len())
        .map_err(|_| InspectError::Allocation)?;
    for origin in catalog.origins() {
        reader.tick()?;
        reader.work(owned.ordinary.pages.len())?;
        if !owned.ordinary.pages.contains(&origin.page()) {
            return Err(InspectError::Malformed.into());
        }
        let descriptor = descriptor::capture(&mut reader, &schema, selected, origin)?;
        let physical_page = u32::try_from(descriptor.physical_offset() / PAGE_BYTES)
            .map_err(|_| InspectError::StructureLimit)?;
        reader.work(owned.ordinary.pages.len())?;
        if !owned.ordinary.pages.contains(&physical_page) {
            return Err(InspectError::Malformed.into());
        }
        reader.work(live.len())?;
        if live
            .iter()
            .any(|old| old.descriptor.physical_offset() == descriptor.physical_offset())
        {
            return Err(InspectError::Unsupported.into());
        }
        reader.work(catalog.catalog().objects().len())?;
        let object = catalog
            .catalog()
            .get(origin.object_id())
            .ok_or(InspectError::Malformed)?;
        if !descriptor.matches_origin(&mut reader, &schema, origin, object.name().as_str())? {
            return Err(InspectError::Malformed.into());
        }
        let traced = descriptor.traced(&mut reader)?;
        for chunk in &traced.chunks {
            reader.work(owned.long_value.pages.len())?;
            if !owned
                .long_value
                .pages
                .contains(&reference_page(chunk.reference))
            {
                return Err(InspectError::Malformed.into());
            }
            for previous in &live {
                reader.work(previous.chunks.len())?;
                if previous
                    .chunks
                    .iter()
                    .any(|old| old.reference == chunk.reference)
                {
                    return Err(InspectError::Unsupported.into());
                }
            }
        }
        live.push(Live {
            origin,
            descriptor,
            kind: traced.kind,
            chunks: traced.chunks,
            bytes: traced.value.as_ref().map_or(0, Vec::len),
        });
    }
    Ok(Ledger {
        _bytes: bytes,
        pages: owned.long_value.pages,
        live,
    })
}

impl Ledger<'_> {
    /// The caller supplies an actual catalog role identity, never a display name.
    /// Other objects may have null/inline bodies, but no other live external ROW.
    pub(crate) fn sole_single(
        &self,
        object: ObjectId,
        expected: u32,
        bytes: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, InspectError> {
        let mut found = false;
        for entry in &self.live {
            check(budget, cancelled)?;
            if entry.origin.object_id() == object {
                if found
                    || !matches!(entry.kind, Kind::Single)
                    || entry.bytes != bytes
                    || entry.chunks.len() != 1
                    || entry.chunks[0].reference != expected
                    || !entry.descriptor.canonical_single(expected, bytes)?
                {
                    return Ok(false);
                }
                found = true;
            } else if matches!(entry.kind, Kind::Single | Kind::Chained) {
                return Ok(false);
            }
        }
        Ok(found)
    }

    /// Current ADD history policy remains sole-user and latest-row-only. The
    /// caller has already checked every row's count, flags and ordinary span.
    pub(crate) fn sole_user_history(
        &self,
        object: ObjectId,
        expected_pages: &[u32],
        last_page: u32,
        last_slot: u8,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, InspectError> {
        check(budget, cancelled)?;
        budget.charge(
            tablecraft_core::limits::Resource::Steps,
            (self.pages.len() + self.live.len()) as u64,
        )?;
        if self.pages != expected_pages {
            return Ok(false);
        }
        let Some(entry) = self
            .live
            .iter()
            .find(|entry| entry.origin.object_id() == object)
        else {
            return Ok(false);
        };
        self.sole_single(
            object,
            reference(last_page, last_slot),
            entry.bytes,
            budget,
            cancelled,
        )
    }
}

#[cfg(test)]
mod tests;
