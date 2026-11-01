//! Source-bound numeric root promotion; no preview append state or candidate.
use super::{InsertRow, NumericKey, SingleProfile, preparation};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    table::indexes::{
        header::IndexHeader,
        mutation::{pages, repack},
        page::LEAF,
    },
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

const ENTRY_BUFFER: usize = size_of::<u8>() + size_of::<i64>() + 2 * size_of::<u32>();

pub(in crate::table) struct RootPromotion {
    pub root: Vec<u8>,
    pub right: Vec<u8>,
    pub left: Vec<u8>,
}

/// Final membership and uniqueness come from the same actual INSERT preparation
/// as existing-page writes. EOF numbers are derived, never caller-selected.
pub(in crate::table) fn prepare_root_promotion(
    image: &NativeImage,
    checked: &TableCheck,
    requests: &[InsertRow<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RootPromotion, EditError> {
    prepare_with(image, checked, requests, false, budget, cancelled)
}

/// The compound normal consumer has already reserved one original EOF DATA
/// page. INDEX numbers still derive from that original image, not caller pages.
pub(in crate::table) fn prepare_root_promotion_after_data(
    image: &NativeImage,
    checked: &TableCheck,
    requests: &[InsertRow<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RootPromotion, EditError> {
    if requests.len() != 1 || requests[0].reference != image.catalog().header().page_count() << 8 {
        return Err(EditError::Preservation);
    }
    prepare_with(image, checked, requests, true, budget, cancelled)
}

fn prepare_with(
    image: &NativeImage,
    checked: &TableCheck,
    requests: &[InsertRow<'_>],
    leading_data: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RootPromotion, EditError> {
    let _span = tracing::debug_span!("native.index.root_promotion.prepare").entered();
    check(budget, cancelled)?;
    let index = super::qualify(checked, 0, budget, cancelled)?;
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let mut position = None;
    for (ordinal, field) in checked.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == key.number() && field.scalar_type() == Some(ScalarType::Int32) {
            position = Some(ordinal);
        }
    }
    let position = position.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
    let profile = SingleProfile {
        column: position,
        kind: NumericKey::new(ScalarType::Int32)?,
        descending: false,
    };
    let prepared = preparation::prepare::<_, ENTRY_BUFFER>(
        image, checked, index, profile, requests, budget, cancelled,
    )?;
    let at = index.root_page() as usize * PAGE_BYTES;
    let original = image
        .as_bytes()
        .get(at..at + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    if IndexHeader::read(original)?.kind != LEAF || !prepared.requires_continuation {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let entries = repack::leaf_entries(&prepared.keys, budget, cancelled)?;
    let middle = entries.len() / 2;
    if middle == 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    for half in [&entries[..middle], &entries[middle..]] {
        if !repack::borrowed_fits_fresh::<_, ENTRY_BUFFER>(
            half,
            &prepared.profile,
            budget,
            cancelled,
        )? {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    let right = image
        .catalog()
        .header()
        .page_count()
        .checked_add(u32::from(leading_data))
        .ok_or(InspectError::StructureLimit)?;
    let left = right.checked_add(1).ok_or(InspectError::StructureLimit)?;
    // These are the working shared preview leaf/root serializers. New normal
    // authority, staging and manual proof remain separate consumers.
    let root = pages::root::<_, ENTRY_BUFFER>(
        original,
        left,
        right,
        entries[middle - 1].value,
        &prepared.profile,
        budget,
        cancelled,
    )?;
    let right_page = pages::leaf::<_, ENTRY_BUFFER>(
        checked.schema().definition_page(),
        left,
        0,
        &entries[middle..],
        &prepared.profile,
        budget,
        cancelled,
    )?;
    let left_page = pages::leaf::<_, ENTRY_BUFFER>(
        checked.schema().definition_page(),
        0,
        right,
        &entries[..middle],
        &prepared.profile,
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(RootPromotion {
        root,
        right: right_page,
        left: left_page,
    })
}
