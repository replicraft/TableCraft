#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "wide_rows/fixture.rs"]
mod fixture;
mod support;
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom},
};
use support::{data_page, put16, put32};
use tablecraft_ace::{
    InspectError, NativeReadSource, TableCursor, TableSnapshotLimits, read_table_projection,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1);
const PROJECTIONS: [Option<&[&str]>; 3] = [None, Some(&["ID"]), Some(&[])];

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

struct Observed<'a> {
    cursor: Cursor<&'a [u8]>,
    calls: &'a Cell<usize>,
}

impl NativeReadSource for Observed<'_> {}

impl Read for Observed<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        self.cursor.read(output)
    }
}

impl Seek for Observed<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.calls.set(self.calls.get() + 1);
        self.cursor.seek(position)
    }
}

#[test]
fn maximum_width_masks_and_variable_values_survive_complete_and_projected_batches() {
    for code in [1, 2, 5, 6] {
        let bytes = fixture::database(code, None);
        for projection in PROJECTIONS {
            let mut input = Cursor::new(&bytes);
            let mut allowance = budget();
            let mut cursor = match projection {
                None => TableCursor::open(&mut input, SCOPE, "Items", &mut allowance, &|| false),
                Some(names) => TableCursor::open_projection(
                    &mut input,
                    SCOPE,
                    "Items",
                    names,
                    &mut allowance,
                    &|| false,
                ),
            }
            .unwrap();
            assert_eq!(cursor.schema().columns().len(), 255);
            for id in 1..=3 {
                let batch = cursor.next_page(1, &mut allowance, &|| false).unwrap();
                assert_eq!(batch.rows(), [fixture::expected(id, projection)]);
                assert_eq!(batch.offset(), (id - 1) as u32);
            }
            assert!(
                cursor
                    .next_page(1, &mut allowance, &|| false)
                    .unwrap()
                    .reached_end()
            );
        }
    }
}

#[test]
fn malformed_wide_suffixes_discard_pending_rows_and_poison_every_projection() {
    // High-slot descending offsets, payload bounds, variable count and row count.
    for damage in [(514, 0), (260, 261), (768, 5), (770, 255), (0, 256)] {
        for code in [1, 2, 5, 6] {
            let bytes = fixture::database(code, Some(damage));
            for projection in PROJECTIONS {
                let calls = Cell::new(0);
                let mut input = Observed {
                    cursor: Cursor::new(&bytes),
                    calls: &calls,
                };
                let mut allowance = budget();
                let mut cursor = match projection {
                    None => {
                        TableCursor::open(&mut input, SCOPE, "Items", &mut allowance, &|| false)
                    }
                    Some(names) => TableCursor::open_projection(
                        &mut input,
                        SCOPE,
                        "Items",
                        names,
                        &mut allowance,
                        &|| false,
                    ),
                }
                .unwrap();
                let prefix = cursor.next_page(1, &mut allowance, &|| false).unwrap();
                assert_eq!(prefix.rows(), [fixture::expected(1, projection)]);
                assert!(
                    cursor.next_page(2, &mut allowance, &|| false).is_err(),
                    "damage={damage:?}, profile={code}, projection={projection:?}"
                );
                assert!(cursor.is_failed());
                assert_eq!(cursor.position(), 1);
                let after_failure = calls.get();
                let remaining = [Resource::Bytes, Resource::Rows, Resource::Steps]
                    .map(|resource| allowance.remaining(resource));
                assert_eq!(
                    cursor.next_page(1, &mut allowance, &|| false).err(),
                    Some(InspectError::CursorFailed)
                );
                assert_eq!(
                    cursor
                        .into_snapshot(TableSnapshotLimits::default(), &mut allowance, &|| false)
                        .err(),
                    Some(InspectError::CursorFailed)
                );
                assert_eq!(calls.get(), after_failure);
                assert_eq!(
                    [Resource::Bytes, Resource::Rows, Resource::Steps]
                        .map(|resource| allowance.remaining(resource)),
                    remaining
                );
            }
            // Even an empty projection validates the complete visible variable tail.
            assert!(
                read_table_projection(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    &[],
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
    }
}
