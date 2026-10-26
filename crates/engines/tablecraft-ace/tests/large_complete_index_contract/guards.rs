use super::*;
use std::{
    cell::Cell,
    io::{self, Read, Seek, SeekFrom},
};
use tablecraft_core::limits::Resource;

const PAGE_BYTES: u64 = 4096;
const BITMAP_PAGE: u64 = 17;
const INDEX_USAGE_PAGE: u64 = 13;

#[test]
fn complete_reads_retain_membership_owner_key_and_late_map_checks() {
    #[derive(Clone, Copy, Debug)]
    enum Fault {
        MissingBit,
        WrongOwner,
        WrongKey,
        LateMap,
    }
    for fault in [
        Fault::MissingBit,
        Fault::WrongOwner,
        Fault::WrongKey,
        Fault::LateMap,
    ] {
        let mut source = fixture::Sparse::new(6, true);
        match fault {
            Fault::MissingBit => source.remove_selected_membership(),
            Fault::WrongOwner => source.wrong_owner(),
            Fault::WrongKey => source.wrong_key(),
            Fault::LateMap => source.bad_second_bitmap(),
        }
        assert_eq!(
            complete(&mut source).err(),
            Some(InspectError::Malformed),
            "fault={fault:?}"
        );
        assert_eq!(
            source.data_reads,
            usize::from(matches!(fault, Fault::WrongOwner | Fault::WrongKey))
        );
    }
}

struct CancelAtBitmap<'a> {
    source: fixture::Sparse,
    position: u64,
    cancelled: &'a Cell<bool>,
    fired: bool,
}

impl NativeReadSource for CancelAtBitmap<'_> {}

impl Read for CancelAtBitmap<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.source.read(bytes)?;
        if count > 0 && self.position / PAGE_BYTES == BITMAP_PAGE {
            self.cancelled.set(true);
            self.fired = true;
        }
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for CancelAtBitmap<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.position = self.source.seek(position)?;
        Ok(self.position)
    }
}

#[test]
fn cancellation_after_usage_bitmap_io_refuses_before_any_row_is_returned() {
    let cancelled = Cell::new(false);
    let mut source = CancelAtBitmap {
        source: fixture::Sparse::new(6, true),
        position: 0,
        cancelled: &cancelled,
        fired: false,
    };
    let result = read_table_index_with_request(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        complete_request(3),
        &mut budget(),
        &|| cancelled.get(),
    );
    assert_eq!(result.err(), Some(InspectError::Cancelled));
    assert!(
        source.fired,
        "cancellation must follow the actual bitmap read"
    );
    assert_eq!(source.source.data_reads, 0);
}

#[test]
fn every_cumulative_allowance_is_enforced_for_explicit_complete_snapshots() {
    for (resource, mut allowance) in [
        (Resource::Bytes, ResourceBudget::new(0, 1024, 1_000_000)),
        (
            Resource::Rows,
            ResourceBudget::new(2 * 1024 * 1024, 0, 1_000_000),
        ),
        (
            Resource::Steps,
            ResourceBudget::new(2 * 1024 * 1024, 1024, 0),
        ),
    ] {
        let mut source = fixture::Sparse::new(6, true);
        let result = read_table_index_with_request(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            complete_request(3),
            &mut allowance,
            &|| false,
        );
        assert!(
            matches!(result.err(), Some(InspectError::Limit(error)) if error.resource == resource)
        );
        assert_eq!(source.data_reads, 0);
    }
}

struct LargeIndexMap {
    source: fixture::Sparse,
    position: u64,
    replacement: Vec<u8>,
    replacements: usize,
}

impl NativeReadSource for LargeIndexMap {}

impl Read for LargeIndexMap {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.source.read(bytes)?;
        if self.position / PAGE_BYTES == INDEX_USAGE_PAGE {
            let offset = (self.position % PAGE_BYTES) as usize;
            bytes[..count].copy_from_slice(&self.replacement[offset..offset + count]);
            self.replacements += 1;
        }
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for LargeIndexMap {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.position = self.source.seek(position)?;
        Ok(self.position)
    }
}

#[test]
fn explicit_complete_opt_in_does_not_expand_the_separate_index_owned_map_ceiling() {
    // Independently encode an inline index ownership row: root 14 plus 4,999
    // additional owned addresses. The table-data ownership remains unchanged.
    let mut map = vec![0_u8; 5 + 5100_usize.div_ceil(8)];
    for page in std::iter::once(14).chain(100..5099) {
        map[5 + page / 8] |= 1 << (page % 8);
    }
    let replacement = crate::support::data_page(1, &[(0, map)]);
    let mut source = LargeIndexMap {
        source: fixture::Sparse::new(6, true),
        position: 0,
        replacement,
        replacements: 0,
    };
    assert_eq!(
        complete(&mut source).err(),
        Some(InspectError::StructureLimit)
    );
    assert!(
        source.replacements > 0,
        "the index map must actually be replaced"
    );
    assert_eq!(source.source.data_reads, 0);
}
