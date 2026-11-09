//! Fitted system-role insertion into the already source-qualified shared ledger.
//! This is physical composition only; creation owns the encoded role and proof.
use super::{catalog, column, maps};
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::{
        page::PAGE_BYTES,
        row::{MAX_SLOTS, SLOT_BYTES, reference, reference_slot, slot_offset},
    },
    header::check,
    table::mutation::owned_lval::{
        maps::{Family, Maps},
        placement::{Plan, geometry},
        property,
    },
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG_PAGE: u32 = 4;
const PROPERTY_PAGE: u32 = 10;
const SOURCE_PAGES: usize = 11;
const DIRECT_CATALOG_SLOTS: usize = 8;
const FORWARDED_CATALOG_SLOTS: usize = DIRECT_CATALOG_SLOTS + 1;

pub(crate) struct RoleAppend {
    pub catalog_bytes: Vec<u8>,
    pub property_bytes: Vec<u8>,
    pub catalog_range: Range<usize>,
    pub property_range: Range<usize>,
    pub root: u32,
    pub property: u32,
}

struct Source {
    catalog: TableSchema,
    maps: Maps,
    catalog_count: usize,
    catalog_end: usize,
    property_count: usize,
    property_end: usize,
}

impl Source {
    fn fits(&self, body_bytes: usize, record_bytes: usize) -> bool {
        self.catalog_count < MAX_SLOTS
            && self.property_count < MAX_SLOTS
            && record_bytes
                .checked_add(SLOT_BYTES)
                .is_some_and(|needed| needed <= self.catalog_end - slot_offset(self.catalog_count))
            && body_bytes.checked_add(SLOT_BYTES).is_some_and(|needed| {
                needed <= self.property_end - slot_offset(self.property_count)
            })
    }
}

fn capture(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    check(budget, cancelled)?;
    if image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES {
        return Err(EditError::Preservation);
    }
    let catalog = catalog(image, budget, cancelled)?;
    let selected = column(&catalog)?;
    let mut input = image.reader();
    let mut reader = crate::format::reader::Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let references = maps::references(&mut reader, &catalog, selected)?;
    let maps = property::capture_maps(image, &catalog, references, budget, cancelled)?;
    if maps.ordinary.pages != [CATALOG_PAGE] || maps.long_value.pages != [PROPERTY_PAGE] {
        return Err(EditError::Preservation);
    }
    let catalog_start = CATALOG_PAGE as usize * PAGE_BYTES;
    let property_start = PROPERTY_PAGE as usize * PAGE_BYTES;
    let (catalog_count, catalog_end) =
        geometry(&image.as_bytes()[catalog_start..catalog_start + PAGE_BYTES])?;
    let (property_count, property_end) =
        geometry(&image.as_bytes()[property_start..property_start + PAGE_BYTES])?;
    if catalog_count != DIRECT_CATALOG_SLOTS && catalog_count != FORWARDED_CATALOG_SLOTS {
        return Err(EditError::Preservation);
    }
    Ok(Source {
        catalog,
        maps,
        catalog_count,
        catalog_end,
        property_count,
        property_end,
    })
}

/// Read actual candidate geometry without appending pages or constructing AFTER.
/// A true result is not source/content admission; the caller still proves both.
pub(crate) fn fits(
    image: &NativeImage,
    body_bytes: usize,
    record_bytes: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    Ok(capture(image, budget, cancelled)?.fits(body_bytes, record_bytes))
}

pub(crate) fn append(
    image: &NativeImage,
    body: &[u8],
    record: &[u8],
    expected_property: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RoleAppend, EditError> {
    let source = capture(image, budget, cancelled)?;
    if expected_property
        != reference(
            PROPERTY_PAGE,
            u8::try_from(source.property_count).map_err(|_| InspectError::Malformed)?,
        )
    {
        return Err(EditError::Preservation);
    }
    if !source.fits(body.len(), record.len()) {
        return Err(EditError::Unsupported(crate::WriteBlocker::RowLayout));
    }
    let Source {
        catalog,
        maps,
        catalog_count,
        catalog_end,
        property_end,
        ..
    } = source;
    let catalog_start = CATALOG_PAGE as usize * PAGE_BYTES;
    let property_start = PROPERTY_PAGE as usize * PAGE_BYTES;
    let mut plan = Plan::new(image, catalog.definition_page(), maps);
    let property_index = plan.append(image, Family::LongValue, body, budget, cancelled)?;
    let catalog_index = plan.append_live_root(image, record, budget, cancelled)?;
    if plan.original_length != image.as_bytes().len()
        || plan.length != image.as_bytes().len()
        || plan.pages.len() != 2
        || plan.appended.len() != 2
        || plan.attempts.len() != 2
        || !plan.maps.edits.is_empty()
        || plan.maps.ordinary.pages != [CATALOG_PAGE]
        || plan.maps.long_value.pages != [PROPERTY_PAGE]
    {
        return Err(EditError::Preservation);
    }
    for (attempt, family, page, bytes) in [
        (
            &plan.attempts[0],
            Family::LongValue,
            PROPERTY_PAGE,
            body.len(),
        ),
        (
            &plan.attempts[1],
            Family::Ordinary,
            CATALOG_PAGE,
            record.len(),
        ),
    ] {
        if attempt.family != family
            || attempt.page != page
            || attempt.bytes != bytes
            || !attempt.fitted
        {
            return Err(EditError::Preservation);
        }
    }
    let property = &plan.appended[property_index];
    let root = &plan.appended[catalog_index];
    if property.family != Family::LongValue
        || property.reference != expected_property
        || property.range
            != (property_start + property_end - body.len()..property_start + property_end)
        || root.family != Family::Ordinary
        || root.reference
            != reference(
                CATALOG_PAGE,
                u8::try_from(catalog_count).map_err(|_| InspectError::Malformed)?,
            )
        || root.range != (catalog_start + catalog_end - record.len()..catalog_start + catalog_end)
    {
        return Err(EditError::Preservation);
    }
    // Detach the range from a borrowed placement event before consuming the plan pages for
    // catalog/property role proof. Only range bounds are copied, not the payload.
    let catalog_range = root.range.clone();
    // Detach the range from a borrowed placement event before consuming the plan pages for
    // catalog/property role proof. Only range bounds are copied, not the payload.
    let property_range = property.range.clone();
    let root = root.reference;
    let property = property.reference;
    let catalog = plan.pages.pop().ok_or(EditError::Preservation)?;
    let lval = plan.pages.pop().ok_or(EditError::Preservation)?;
    for (changed, family, number) in [
        (&catalog, Family::Ordinary, CATALOG_PAGE),
        (&lval, Family::LongValue, PROPERTY_PAGE),
    ] {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (2 * PAGE_BYTES) as u64)
            .map_err(InspectError::from)?;
        let at = number as usize * PAGE_BYTES;
        if changed.family != family
            || changed.number != number
            || changed.before != image.as_bytes()[at..at + PAGE_BYTES]
        {
            return Err(EditError::Preservation);
        }
    }
    let word = crate::format::bytes::u16_at(
        &catalog.after,
        crate::format::row::slot_offset(usize::from(reference_slot(root))),
    )?;
    if word & !crate::format::row::OFFSET_MASK != 0 {
        return Err(EditError::Preservation);
    }
    Ok(RoleAppend {
        catalog_bytes: catalog.after,
        property_bytes: lval.after,
        catalog_range,
        property_range,
        root,
        property,
    })
}
