//! Selected reads validate large owned maps without scanning unrelated payloads.
use super::*;
use std::{
    cell::Cell,
    io::{Seek, SeekFrom},
};

const PAGE_BYTES: usize = 4096;

impl fixture::Sparse {
    fn large_index_owned_map(&mut self, indirect: bool) {
        // Root 14 plus 4,096 other owned pages. Only the root is relevant to the
        // selected key; unrelated owned pages must not be scanned implicitly.
        let mut bits = vec![0; 4196_usize.div_ceil(8)];
        bits[14 / 8] |= 1 << (14 % 8);
        for number in 100..4196 {
            bits[number / 8] |= 1 << (number % 8);
        }
        let map = if indirect {
            self.prefix[19 * PAGE_BYTES..19 * PAGE_BYTES + 4].copy_from_slice(&[5, 1, 0, 0]);
            self.prefix[19 * PAGE_BYTES + 4..19 * PAGE_BYTES + 4 + bits.len()]
                .copy_from_slice(&bits);
            [vec![1], 19_u32.to_le_bytes().to_vec()].concat()
        } else {
            [vec![0; 5], bits].concat()
        };
        self.prefix[13 * PAGE_BYTES..14 * PAGE_BYTES]
            .copy_from_slice(&support::data_page(1, &[(0, map)]));
    }

    fn late_invalid_index_bitmap(&mut self, outside_source: bool) {
        let mut map = [vec![1], 19_u32.to_le_bytes().to_vec()].concat();
        if outside_source {
            map.extend(0_u32.to_le_bytes());
        }
        map.extend(20_u32.to_le_bytes());
        self.prefix[13 * PAGE_BYTES..14 * PAGE_BYTES]
            .copy_from_slice(&support::data_page(1, &[(0, map)]));
        let source_pages = self.seek(SeekFrom::End(0)).unwrap() as usize / PAGE_BYTES;
        let second = &mut self.prefix[20 * PAGE_BYTES..21 * PAGE_BYTES];
        if outside_source {
            second[..4].copy_from_slice(&[5, 1, 0, 0]);
            let relative = source_pages - 2 * 32736;
            second[4 + relative / 8] |= 1 << (relative % 8);
        } else {
            second[0] = 255;
        }
    }
}

fn selected(
    source: &mut fixture::Sparse,
    pool: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Value>>, InspectError> {
    read_table_index_range(
        source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        Int32IndexSelection::Range {
            lower: Included(1),
            upper: Included(1),
        },
        pool,
        cancelled,
    )
    .map(|snapshot| snapshot.into_parts().2)
}

#[test]
fn selected_row_reads_accept_4097_inline_and_indirect_index_owned_pages() {
    for code in [1, 2, 3, 5, 6] {
        for indirect in [false, true] {
            let mut source = fixture::Sparse::new(code, true);
            source.large_index_owned_map(indirect);
            // NO_CLONE: The source is mutably consumed during the read; retain an
            // independent pre-read raw-prefix oracle to check preservation.
            let before = source.prefix.to_vec();
            assert_eq!(
                selected(&mut source, &mut budget(), &|| false)
                    .unwrap_or_else(|error| panic!("code={code}, indirect={indirect}: {error:?}")),
                [vec![Value::Int32(1), Value::Int32(10)]]
            );
            assert_eq!(source.data_reads, 1);
            assert_eq!(source.prefix, before);
            assert!(source.bytes_read < 256 * 1024);
        }
    }
}

#[test]
fn corruption_after_4097_owned_members_never_publishes_a_selected_row() {
    for outside_source in [false, true] {
        let mut source = fixture::Sparse::new(6, true);
        source.large_index_owned_map(true);
        source.late_invalid_index_bitmap(outside_source);
        assert_eq!(
            selected(&mut source, &mut budget(), &|| false),
            Err(InspectError::Malformed)
        );
        assert_eq!(source.data_reads, 0);
    }
}

#[test]
fn large_owned_map_reads_keep_explicit_budgets_and_terminal_cancellation() {
    let mut source = fixture::Sparse::new(6, true);
    source.large_index_owned_map(true);
    let observed = Cell::new(0);
    selected(&mut source, &mut budget(), &|| {
        observed.set(observed.get() + 1);
        false
    })
    .unwrap();
    for stop in [1, observed.get() / 2, observed.get()] {
        let mut source = fixture::Sparse::new(6, true);
        source.large_index_owned_map(true);
        let checks = Cell::new(0);
        assert_eq!(
            selected(&mut source, &mut budget(), &|| {
                checks.set(checks.get() + 1);
                checks.get() >= stop
            }),
            Err(InspectError::Cancelled)
        );
    }
    for mut limited in [
        ResourceBudget::new(16_384, 1024, 1_000_000),
        ResourceBudget::new(2 * 1024 * 1024, 1024, 100),
    ] {
        let mut source = fixture::Sparse::new(6, true);
        source.large_index_owned_map(true);
        assert!(matches!(
            selected(&mut source, &mut limited, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
}
