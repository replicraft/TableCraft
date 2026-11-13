//! Independent fresh source, literal system declaration and protected complement.
use super::source;
use crate::{
    NativeImage, QueryCreationDefinition, QueryCreationError,
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES},
    header::check,
    table::mutation::query_creation::{append, proof, source as existing},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn verify(
    before: &NativeImage,
    after: &NativeImage,
    requested: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    // Re-observe the original independently, including its full physical census
    // and deterministic allocation. No producer append spans/certificate enter.
    let original = source::capture(before, budget, cancelled)?;
    let fresh = existing::capture_after(after, budget, cancelled)?;
    if fresh.parent != original.parent
        || fresh.catalog.definition_page() != original.catalog.definition_page()
        || fresh.catalog_page != original.catalog_page
        || fresh.queries.definition_page() != original.definition_page
        || fresh.query_page != original.data_page
        || fresh.queries.declared_rows() != 0
        || original.catalog.declared_rows().checked_add(1) != Some(fresh.catalog.declared_rows())
        || after.as_bytes().len() != before.as_bytes().len() + 2 * PAGE_BYTES
    {
        return Err(QueryCreationError::Preservation);
    }
    original
        .allocation
        .verify_after(after.as_bytes(), budget, cancelled)
        .map_err(source::allocation_error)?;
    let old_catalog = proof::capture(
        before,
        original.catalog_page,
        original.catalog.definition_page(),
        budget,
        cancelled,
    )?;
    let new_catalog = proof::capture(
        after,
        fresh.catalog_page,
        fresh.catalog.definition_page(),
        budget,
        cancelled,
    )?;
    let old_maps = proof::capture(before, original.map_page, 0, budget, cancelled)?;
    let new_maps = proof::capture(after, original.map_page, 0, budget, cancelled)?;
    budget.charge(
        Resource::Steps,
        (old_catalog.count + old_maps.count) as u64 * 2,
    )?;
    if new_catalog.count != old_catalog.count + 1
        || new_maps.count != old_maps.count + 4
        || new_catalog.records[..old_catalog.count] != old_catalog.records
        || new_maps.records[..old_maps.count] != old_maps.records
    {
        return Err(QueryCreationError::Preservation);
    }
    proof::verify_storage_record(
        after,
        &fresh.catalog,
        proof::record(
            after,
            fresh.catalog_page,
            &new_catalog.records[old_catalog.count],
        )?,
        proof::StorageRow {
            parent: original.parent,
            native_id: i32::try_from(original.definition_page)
                .map_err(|_| QueryCreationError::Preservation)?,
            definition: requested,
        },
        budget,
        cancelled,
    )?;
    verify_definition(after, &original, budget, cancelled)?;
    verify_empty_data(after, &original, budget, cancelled)?;
    for ordinal in 0..4 {
        let row = proof::record(
            after,
            original.map_page,
            &new_maps.records[old_maps.count + ordinal],
        )?;
        let mut expected = [0_u8; 13];
        if ordinal < 2 {
            let base = original.data_page / 64 * 64;
            expected[1..5].copy_from_slice(&base.to_le_bytes());
            let bit = original.data_page - base;
            expected[5 + bit as usize / 8] = 1 << (bit % 8);
        }
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, row.len().max(expected.len()) as u64)?;
        if row != expected {
            return Err(QueryCreationError::Preservation);
        }
    }
    let count = original.catalog.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    for (at, (&old, &new)) in before.as_bytes().iter().zip(after.as_bytes()).enumerate() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        if old == new {
            continue;
        }
        let allocator = original
            .allocation
            .allowed_before_ranges()
            .iter()
            .any(|range| range.contains(&at));
        let page = at / PAGE_BYTES;
        let offset = at % PAGE_BYTES;
        let allowed = allocator
            || (count..count + 4).contains(&at)
            || page == original.catalog_page as usize
                && proof::append_scope(offset, &old_catalog, &new_catalog)
            || page == original.map_page as usize
                && proof::append_scope(offset, &old_maps, &new_maps);
        if !allowed {
            return Err(QueryCreationError::Preservation);
        }
    }
    Ok(())
}

/// Every byte in a new page is either an independently checked fact or zero.
struct LiteralPage<'a> {
    bytes: &'a [u8],
    covered: [bool; PAGE_BYTES],
}

impl<'a> LiteralPage<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, QueryCreationError> {
        if bytes.len() != PAGE_BYTES {
            return Err(QueryCreationError::Preservation);
        }
        Ok(Self {
            bytes,
            covered: [false; PAGE_BYTES],
        })
    }

    fn field(
        &mut self,
        at: usize,
        expected: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryCreationError> {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, expected.len() as u64)?;
        let end = at
            .checked_add(expected.len())
            .filter(|end| *end <= PAGE_BYTES)
            .ok_or(QueryCreationError::Preservation)?;
        if self.bytes[at..end] != *expected || self.covered[at..end].iter().any(|value| *value) {
            return Err(QueryCreationError::Preservation);
        }
        self.covered[at..end].fill(true);
        Ok(())
    }

    fn finish(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryCreationError> {
        for (&byte, covered) in self.bytes.iter().zip(self.covered) {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, 1)?;
            if !covered && byte != 0 {
                return Err(QueryCreationError::Preservation);
            }
        }
        Ok(())
    }
}

fn verify_empty_data(
    after: &NativeImage,
    source: &source::Source<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    let mut page = LiteralPage::new(append::page(after.as_bytes(), source.data_page)?)?;
    page.field(0, &[1, 1], budget, cancelled)?;
    page.field(2, &4082_u16.to_le_bytes(), budget, cancelled)?;
    page.field(4, &source.definition_page.to_le_bytes(), budget, cancelled)?;
    page.finish(budget, cancelled)
}

fn verify_definition(
    after: &NativeImage,
    source: &source::Source<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    // Independent expected metadata. Neither the constructive Field list nor
    // definition writer is used in the proof of the fresh declaration bytes.
    let columns = [
        ("ObjectId", 4_u8, 0_u16, 4_u16, 0_u16, true),
        ("Attribute", 2, 4, 1, 0, true),
        ("Order", 9, 0, 4, 0, false),
        ("Name1", 10, 0, 510, 1, false),
        ("Name2", 10, 0, 510, 2, false),
        ("Expression", 12, 0, 0, 3, false),
        ("Flag", 3, 5, 2, 4, true),
        ("LvExtra", 4, 7, 4, 4, true),
    ];
    let names_bytes: usize = columns
        .iter()
        .map(|(name, ..)| 2 + name.encode_utf16().count() * 2)
        .sum();
    let length = 63 + columns.len() * 25 + names_bytes + 10 + 2;
    let maps = source.map_references()?;
    let mut page = LiteralPage::new(append::page(after.as_bytes(), source.definition_page)?)?;
    page.field(0, &[2, 1], budget, cancelled)?;
    page.field(
        2,
        &u16::try_from(PAGE_BYTES - length - 8)
            .map_err(|_| QueryCreationError::Preservation)?
            .to_le_bytes(),
        budget,
        cancelled,
    )?;
    page.field(8, &(length as u32).to_le_bytes(), budget, cancelled)?;
    page.field(12, &1625_u32.to_le_bytes(), budget, cancelled)?;
    page.field(24, &[1], budget, cancelled)?;
    page.field(40, &[0x53], budget, cancelled)?;
    for (at, value) in [(41, 8_u16), (43, 4), (45, 8)] {
        page.field(at, &value.to_le_bytes(), budget, cancelled)?;
    }
    page.field(55, &maps[0].to_le_bytes(), budget, cancelled)?;
    page.field(59, &maps[1].to_le_bytes(), budget, cancelled)?;
    let mut name_at = 63 + columns.len() * 25;
    for (number, (name, kind, offset, width, variable, fixed)) in columns.into_iter().enumerate() {
        let at = 63 + number * 25;
        page.field(at, &[kind], budget, cancelled)?;
        page.field(at + 1, &1625_u32.to_le_bytes(), budget, cancelled)?;
        page.field(at + 5, &(number as u16).to_le_bytes(), budget, cancelled)?;
        page.field(at + 7, &variable.to_le_bytes(), budget, cancelled)?;
        page.field(at + 9, &(number as u16).to_le_bytes(), budget, cancelled)?;
        if kind == 10 {
            page.field(at + 11, &source.order.native_bytes(), budget, cancelled)?;
        }
        page.field(at + 15, &[if fixed { 3 } else { 2 }], budget, cancelled)?;
        page.field(at + 21, &offset.to_le_bytes(), budget, cancelled)?;
        page.field(at + 23, &width.to_le_bytes(), budget, cancelled)?;
        page.field(
            name_at,
            &((name.encode_utf16().count() * 2) as u16).to_le_bytes(),
            budget,
            cancelled,
        )?;
        name_at += 2;
        for unit in name.encode_utf16() {
            page.field(name_at, &unit.to_le_bytes(), budget, cancelled)?;
            name_at += 2;
        }
    }
    page.field(name_at, &5_u16.to_le_bytes(), budget, cancelled)?;
    page.field(name_at + 2, &maps[2].to_le_bytes(), budget, cancelled)?;
    page.field(name_at + 6, &maps[3].to_le_bytes(), budget, cancelled)?;
    page.field(name_at + 10, &u16::MAX.to_le_bytes(), budget, cancelled)?;
    page.finish(budget, cancelled)
}
