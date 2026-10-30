//! Independently certified DATA frontier shared by real continuation consumers.
use super::{ExpectedByte, ReplayAuthority, record::RecordImage, replay};
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::variable_update::{audit, placement::DataView},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) struct CertifiedPlacement<'a> {
    image: &'a NativeImage,
    after: &'a [u8],
    end: u32,
    edits: Vec<ExpectedByte>,
    pages: Vec<u32>,
}

impl CertifiedPlacement<'_> {
    pub(in crate::table) fn require_source(&self, image: &NativeImage) -> Result<(), EditError> {
        if !std::ptr::eq(self.image, image) {
            return Err(EditError::Preservation);
        }

        Ok(())
    }

    pub(in crate::table) fn require_after(&self, after: &[u8]) -> Result<(), EditError> {
        if !std::ptr::eq(self.after, after) {
            return Err(EditError::Preservation);
        }

        Ok(())
    }

    pub(in crate::table) fn end(&self) -> u32 {
        self.end
    }

    pub(in crate::table) fn pages(&self) -> &[u32] {
        &self.pages
    }

    pub(in crate::table) fn edits(&self) -> impl Iterator<Item = (usize, u8, u8)> + '_ {
        self.edits
            .iter()
            .map(|byte| (byte.offset, byte.before, byte.after))
    }
}

pub(in crate::table::mutation) struct Operation<'a, 'proof, R> {
    pub image: &'a NativeImage,
    pub schema: &'proof TableSchema,
    pub records: &'proof [R],
    pub authority: ReplayAuthority<'proof>,
}

pub(in crate::table::mutation) fn prove<'a, R: RecordImage>(
    operation: Operation<'a, '_, R>,
    data: DataView<'_>,
    after: &'a [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CertifiedPlacement<'a>, EditError> {
    check(budget, cancelled)?;
    let image = operation.image;
    if data.append.original_length != image.as_bytes().len()
        || data.append.length != after.len()
        || data.length > after.len()
    {
        return Err(EditError::Preservation);
    }

    let metadata = operation
        .authority
        .metadata()
        .ok_or(EditError::Preservation)?;
    metadata.require_source(image, operation.schema.definition)?;
    super::record::compare(operation.records, data, budget, cancelled)?;
    let replay = replay(
        image,
        operation.schema,
        data,
        operation.records,
        operation.authority,
        budget,
        cancelled,
    )?;

    budget
        .charge_allocation(data.pages.len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(data.pages.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut added = 0;

    for page in data.pages {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (seen.len() + 2 * PAGE_BYTES) as u64)
            .map_err(InspectError::from)?;
        metadata.require_data_page(page.number, budget, cancelled)?;
        if seen.contains(&page.number) || page.after.len() != PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        seen.push(page.number);

        let start = (page.number as usize)
            .checked_mul(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let end = start
            .checked_add(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if page.before.is_empty() {
            let levels = replay.data_pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
            budget
                .charge(Resource::Steps, levels as u64)
                .map_err(InspectError::from)?;
            if replay.data_pages.binary_search(&page.number).is_err() {
                return Err(EditError::Preservation);
            }
            added += 1;
        } else if page.before.len() != PAGE_BYTES
            || image.as_bytes().get(start..end) != Some(page.before.as_slice())
        {
            return Err(EditError::Preservation);
        }

        if after.get(start..end) != Some(page.after.as_slice()) {
            return Err(EditError::Preservation);
        }
    }
    if added != replay.data_pages.len() {
        return Err(EditError::Preservation);
    }

    let mut metadata_pages = data.maps.metadata_pages();
    match (
        &replay.metadata,
        metadata_pages.next(),
        metadata_pages.next(),
    ) {
        (None, None, None) => {}
        (Some(expected), Some((number, reference, bytes)), None) => {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (seen.len() + 2 * PAGE_BYTES) as u64)
                .map_err(InspectError::from)?;
            if number != expected.number
                || reference != expected.reference
                || bytes != expected.bytes.as_slice()
                || seen.contains(&number)
            {
                return Err(EditError::Preservation);
            }
            data.append.require_candidate_bitmap(
                number,
                operation.schema.definition,
                reference,
                budget,
                cancelled,
            )?;
            let start = (number as usize)
                .checked_mul(PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let end = start
                .checked_add(PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            if after.get(start..end) != Some(expected.bytes.as_slice()) {
                return Err(EditError::Preservation);
            }
        }
        _ => return Err(EditError::Preservation),
    }

    // This framing replay now sees exact original before-pages, certified
    // record identities/lengths, and the actual composed DATA bytes.
    audit::pages(data, budget, cancelled)?;
    check(budget, cancelled)?;

    Ok(CertifiedPlacement {
        image,
        after,
        end: replay.end,
        edits: replay.edits,
        pages: replay.phase_pages,
    })
}
