//! All physical memberships are prepared before a coordinated two-root promotion.
use super::{
    IndexInsertion, InsertRow, NumericKey, SingleProfile, preparation, root_split::RootPromotion,
};
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

const ENTRY_BUFFER: usize = size_of::<u8>() + size_of::<i64>() + 2 * size_of::<u32>();

pub(in crate::table) struct Coordinated {
    pub primary: IndexInsertion,
    pub wide: [RootPromotion; 2],
    pub distinct_added: [usize; 3],
}

pub(in crate::table) fn prepare(
    image: &NativeImage,
    checked: &TableCheck,
    requests: &[InsertRow<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Coordinated, EditError> {
    check(budget, cancelled)?;
    if checked.schema().layout.physical_indexes != 3 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut primary = None;
    let mut first = None;
    let mut second = None;
    let mut distinct_added = [0; 3];
    for physical in 0..3_u32 {
        check(budget, cancelled)?;
        let index = super::qualify_actions(checked, physical, budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        let mut column = None;
        for (position, field) in checked.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            if field.number() == key.number() {
                column = Some((position, field));
            }
        }
        let (column, field) = column.ok_or(EditError::Preservation)?;
        let profile = SingleProfile {
            column,
            kind: NumericKey::new(field.scalar_type().ok_or(InspectError::Unsupported)?)?,
            descending: key.descending(),
        };
        let prepared = preparation::prepare::<_, ENTRY_BUFFER>(
            image, checked, index, profile, requests, budget, cancelled,
        )?;
        distinct_added[physical as usize] = prepared.result.distinct_added;
        if physical == 0 {
            primary = Some(prepared.finish_fitting::<ENTRY_BUFFER>(budget, cancelled)?);
            continue;
        }
        let base = index.root_page() as usize * PAGE_BYTES;
        let original = image
            .as_bytes()
            .get(base..base + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if IndexHeader::read(original)?.kind != LEAF || !prepared.requires_continuation {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let entries = repack::leaf_entries(&prepared.keys, budget, cancelled)?;
        let middle = entries.len() / 2;
        if middle == 0 {
            return Err(EditError::Preservation);
        }
        for half in [&entries[..middle], &entries[middle..]] {
            if !repack::borrowed_fits_fresh_with_policy::<_, ENTRY_BUFFER>(
                half,
                &prepared.profile,
                repack::PrefixPolicy::KeyComponent,
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
            .checked_add((physical - 1) * 2)
            .ok_or(InspectError::StructureLimit)?;
        let left = right.checked_add(1).ok_or(InspectError::StructureLimit)?;
        let root = pages::root::<_, ENTRY_BUFFER>(
            original,
            left,
            right,
            entries[middle - 1].value,
            &prepared.profile,
            budget,
            cancelled,
        )?;
        let right_page = pages::leaf_with_policy::<_, ENTRY_BUFFER>(
            &pages::LeafHeader {
                owner: checked.schema().definition_page(),
                links: (left, 0),
            },
            &entries[middle..],
            &prepared.profile,
            repack::PrefixPolicy::KeyComponent,
            budget,
            cancelled,
        )?;
        let left_page = pages::leaf_with_policy::<_, ENTRY_BUFFER>(
            &pages::LeafHeader {
                owner: checked.schema().definition_page(),
                links: (0, right),
            },
            &entries[..middle],
            &prepared.profile,
            repack::PrefixPolicy::KeyComponent,
            budget,
            cancelled,
        )?;
        let promotion = RootPromotion {
            root,
            right: right_page,
            left: left_page,
        };
        if physical == 1 {
            first = Some(promotion);
        } else {
            second = Some(promotion);
        }
    }
    check(budget, cancelled)?;
    Ok(Coordinated {
        primary: primary.ok_or(EditError::Preservation)?,
        wide: [
            first.ok_or(EditError::Preservation)?,
            second.ok_or(EditError::Preservation)?,
        ],
        distinct_added,
    })
}
