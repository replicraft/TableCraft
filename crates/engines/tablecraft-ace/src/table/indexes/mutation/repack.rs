//! Repack entry data within an existing page; never allocate or relink pages.
use super::super::{
    header::IndexHeader,
    page::{ENTRY_START, Entry},
};
#[cfg(feature = "ace16-write-preview")]
use super::retained::RetainedProfile;
use super::{IndexDataPatch, encoding, profile::WriteProfile};
use crate::{EditError, InspectError, WriteBlocker, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const BITMAP_START: usize = size_of::<IndexHeader>();
const BITMAP_BITS: usize = u8::BITS as usize;

pub(super) fn page<P: WriteProfile, const N: usize>(
    original: &[u8],
    entries: &[Entry<P::Key>],
    profile: &P,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    page_with::<_, N>(
        original,
        entries,
        |entry| encoding::entry::<P, N>(entry, profile),
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn retained_page<P: RetainedProfile, const N: usize>(
    original: &[u8],
    entries: &[Entry<P::Key>],
    profile: &P,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    page_with::<_, N>(
        original,
        entries,
        |entry| encoding::retained_entry::<P, N>(entry, profile),
        budget,
        cancelled,
    )
}

struct Layout {
    prefix: usize,
    length: usize,
}

fn measure<K, const N: usize>(
    entries: &[Entry<K>],
    original_prefix: usize,
    encode: &impl Fn(&Entry<K>) -> Result<encoding::Encoded<N>, InspectError>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Layout, EditError> {
    let mut first = [0; N];
    let mut common = 0;
    let mut total = 0_usize;
    for (position, entry) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, N as u64)
            .map_err(InspectError::from)?;
        let encoded = encode(entry)?;
        total = total
            .checked_add(encoded.length)
            .ok_or(InspectError::StructureLimit)?;
        if position == 0 {
            first = encoded.bytes;
            common = if entries.len() == 1 {
                encoded.length
            } else {
                encoded.key_length
            };
        } else {
            common = common.min(encoded.key_length);
            common = first[..common]
                .iter()
                .zip(&encoded.bytes[..common])
                .take_while(|(a, b)| a == b)
                .count();
        }
    }
    let saved = entries.len().saturating_sub(1);
    let mut prefix = original_prefix.min(common);
    let capacity = PAGE_BYTES - ENTRY_START;
    if total - prefix * saved > capacity {
        prefix = common;
    }
    let length = total - prefix * saved;
    Ok(Layout { prefix, length })
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn fits<P: WriteProfile, const N: usize>(
    original: &[u8],
    entries: &[Entry<P::Key>],
    profile: &P,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let header = IndexHeader::read(original)?;
    let layout = measure(
        entries,
        usize::from(header.prefix.get()),
        &|entry| encoding::entry::<P, N>(entry, profile),
        budget,
        cancelled,
    )?;
    Ok(layout.length <= PAGE_BYTES - ENTRY_START)
}

fn page_with<K, const N: usize>(
    original: &[u8],
    entries: &[Entry<K>],
    encode: impl Fn(&Entry<K>) -> Result<encoding::Encoded<N>, InspectError>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    if original.len() != PAGE_BYTES {
        return Err(EditError::Preservation);
    }
    let header = IndexHeader::read(original)?;
    let Layout { prefix, length } = measure(
        entries,
        usize::from(header.prefix.get()),
        &encode,
        budget,
        cancelled,
    )?;
    let capacity = PAGE_BYTES - ENTRY_START;
    if length > capacity {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    after.extend_from_slice(original);
    after[BITMAP_START..ENTRY_START].fill(0);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, N as u64)
            .map_err(InspectError::from)?;
        let encoded = encode(entry)?;
        let shared = if position == 0 { 0 } else { prefix };
        let next = end + encoded.length - shared;
        after[ENTRY_START + end..ENTRY_START + next]
            .copy_from_slice(&encoded.bytes[shared..encoded.length]);
        after[BITMAP_START + next / BITMAP_BITS] |= 1 << (next % BITMAP_BITS);
        end = next;
    }
    if end != length {
        return Err(EditError::Preservation);
    }
    let free = u16::try_from(capacity - length).map_err(|_| EditError::Preservation)?;
    let prefix = u16::try_from(prefix).map_err(|_| EditError::Preservation)?;
    after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(&free.to_le_bytes());
    after[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()].copy_from_slice(&prefix.to_le_bytes());
    check(budget, cancelled)?;
    Ok(after)
}

pub(super) fn patches(
    original: &[u8],
    after: &[u8],
    number: u32,
    patches: &mut Vec<IndexDataPatch>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let old_end = end(original)?;
    let new_end = end(after)?;
    for range in [
        FREE_OFFSET..FREE_OFFSET + size_of::<u16>(),
        PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>(),
        BITMAP_START..old_end.max(new_end),
    ] {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, range.len() as u64)
            .map_err(InspectError::from)?;
        if original[range.clone()] == after[range.clone()] {
            continue;
        }
        budget
            .charge_allocation(2 * range.len() as u64, 1)
            .map_err(InspectError::from)?;
        let mut before = Vec::new();
        let mut replacement = Vec::new();
        before
            .try_reserve_exact(range.len())
            .map_err(|_| InspectError::Allocation)?;
        replacement
            .try_reserve_exact(range.len())
            .map_err(|_| InspectError::Allocation)?;
        before.extend_from_slice(&original[range.clone()]);
        replacement.extend_from_slice(&after[range.clone()]);
        budget
            .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
            .map_err(InspectError::from)?;
        patches
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        patches.push(IndexDataPatch {
            offset: number as usize * PAGE_BYTES + range.start,
            before,
            after: replacement,
        });
    }
    Ok(())
}

fn end(page: &[u8]) -> Result<usize, InspectError> {
    if page.len() != PAGE_BYTES {
        return Err(InspectError::Malformed);
    }
    PAGE_BYTES
        .checked_sub(usize::from(IndexHeader::read(page)?.free.get()))
        .filter(|end| *end >= ENTRY_START)
        .ok_or(InspectError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::super::super::page::KeyRef;
    use super::*;
    fn budget() -> ResourceBudget {
        ResourceBudget::new(1024 * 1024, 10_000, 100_000)
    }
    use super::super::{NumericKey, profile::SingleProfile};
    fn page(
        original: &[u8],
        entries: &[Entry],
        kind: NumericKey,
        descending: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        super::page::<_, { encoding::MAX_ENTRY_BYTES }>(
            original,
            entries,
            &SingleProfile {
                column: 0,
                kind,
                descending,
            },
            budget,
            cancelled,
        )
    }
    fn int32() -> NumericKey {
        NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap()
    }
    fn original() -> Vec<u8> {
        let mut bytes = vec![0xa7; PAGE_BYTES];
        bytes[..ENTRY_START].fill(0);
        bytes[..8].copy_from_slice(&[4, 1, 0x20, 0x0e, 10, 0, 0, 0]);
        bytes
    }
    fn key(key: Option<i64>, row: u32) -> Entry {
        Entry {
            value: KeyRef { key, row },
            child: 0,
        }
    }
    #[test]
    fn literal_nullable_page_updates_exact_free_bitmap_payload_and_preserves_padding() {
        let original = original();
        let entries = [key(None, 0x0c00), key(Some(20), 0x0c01)];
        let after = page(&original, &entries, int32(), false, &mut budget(), &|| {
            false
        })
        .unwrap();
        let mut expected = original.clone();
        expected[2..4].copy_from_slice(&[0x12, 0x0e]);
        expected[27] = 0x20;
        expected[28] = 0x40;
        expected[480..494].copy_from_slice(&[0, 0, 0, 12, 0, 0x7f, 0x80, 0, 0, 20, 0, 0, 12, 1]);
        assert_eq!(after, expected);
        let grown = page(
            &after,
            &[key(Some(10), 0x0c00), key(Some(20), 0x0c01)],
            int32(),
            false,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(&grown[2..4], &[0x0e, 0x0e]);
        assert_eq!(&grown[27..30], &[0, 2, 4]);
        assert_eq!(&grown[498..], &original[498..]);
        let mut patches_out = Vec::new();
        patches(
            &original,
            &after,
            13,
            &mut patches_out,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(patches_out.len(), 2);
        assert_eq!(patches_out[0].offset, 13 * 4096 + 2);
        assert_eq!(patches_out[1].offset, 13 * 4096 + 27);
    }
    #[test]
    fn compressed_capacity_uses_a_valid_common_prefix_and_rejects_a_required_split() {
        let original = original();
        let entries = (0..600)
            .map(|v| key(Some(v), 0x0c00 + v as u32))
            .collect::<Vec<_>>();
        let after = page(&original, &entries, int32(), false, &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(u16::from_le_bytes(after[24..26].try_into().unwrap()), 3);
        assert_eq!(u16::from_le_bytes(after[2..4].try_into().unwrap()), 13);
        let entries = (0..1000)
            .map(|v| key(Some(v), 0x0c00 + v as u32))
            .collect::<Vec<_>>();
        assert_eq!(
            page(&original, &entries, int32(), false, &mut budget(), &|| {
                false
            })
            .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    #[test]
    fn single_node_entry_retains_a_valid_unused_prefix_through_child_bytes() {
        let mut original = original();
        original[0] = 3;
        original[26] = 1;
        original[20] = 16;
        original[2..4].copy_from_slice(&[0x13, 0x0e]);
        original[24] = 12;
        original[28] = 0x20;
        original[480..493].copy_from_slice(&[0x7f, 0x80, 0, 0, 1, 0, 0, 12, 0, 0, 0, 0, 15]);
        let entries = [Entry {
            value: KeyRef {
                key: Some(1),
                row: 0x0c00,
            },
            child: 15,
        }];
        assert_eq!(
            page(&original, &entries, int32(), false, &mut budget(), &|| {
                false
            })
            .unwrap(),
            original
        );
    }
    #[test]
    fn invalid_patch_geometry_and_cancellation_return_no_repacked_page() {
        let original = original();
        assert_eq!(
            page(&original, &[], int32(), false, &mut budget(), &|| true).err(),
            Some(EditError::Inspect(InspectError::Cancelled))
        );
        let mut invalid = original.clone();
        invalid[2..4].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            patches(
                &invalid,
                &original,
                13,
                &mut Vec::new(),
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
    }
}
