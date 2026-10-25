#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
#[path = "../../../engines/tablecraft-ace/tests/large_maps/fixture.rs"]
mod map_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

#[path = "progressive_source/extrema.rs"]
mod extrema;
#[path = "progressive_source/union.rs"]
mod union;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, Source};

const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const ROW_COUNT_OFFSET: usize = 16;
const FIRST_FIELD: usize = 63;
const FIELD_BYTES: usize = 25;
const SCOPE: CatalogScope = CatalogScope::new(1);

struct Watched {
    input: Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
    failed_page: Rc<Cell<Option<usize>>>,
    changed_schema: Rc<Cell<bool>>,
}

impl NativeReadSource for Watched {}

impl Read for Watched {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let position = self.input.position() as usize;
        let page = position / PAGE_BYTES;
        if self.failed_page.get() == Some(page) {
            return Err(std::io::Error::other("injected read failure"));
        }
        self.reads.set(self.reads.get() + 1);
        let count = self.input.read(bytes)?;
        let score_type = TABLE_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES;
        if self.changed_schema.get() && (position..position + count).contains(&score_type) {
            bytes[score_type - position] = 6; // Single replaces the bound Int32.
        }
        Ok(count)
    }
}

impl Seek for Watched {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.input.seek(position)
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000)
}

fn watched(bytes: Vec<u8>) -> Watched {
    Watched {
        input: Cursor::new(bytes),
        reads: Rc::new(Cell::new(0)),
        failed_page: Rc::new(Cell::new(None)),
        changed_schema: Rc::new(Cell::new(false)),
    }
}

fn source(input: Watched, batch: u32) -> NativeSource<'static, Watched> {
    NativeSource::new_with_snapshot_limits(
        input,
        SCOPE,
        budget(),
        &|| false,
        TableSnapshotLimits::new(1, batch).unwrap(),
    )
    .unwrap()
}

fn collect(source: &impl Source, indices: &[usize]) -> Result<Vec<Vec<Value>>, QueryError> {
    let mut rows = Vec::new();
    source.scan_projected_progressive("Items", indices, &mut |row| {
        rows.push(
            (0..row.len())
                .map(|index| row.get(index).unwrap().clone())
                .collect(),
        );
        Ok(())
    })?;
    Ok(rows)
}

#[test]
fn ordinary_projection_streams_multiple_batches_without_retention_ceiling_or_cache() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = map_fixture::database(7, true, false);
        bytes[20] = profile;
        if profile != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let input = watched(bytes);
        let reads = input.reads.clone();
        let source = source(input, 2);
        assert_eq!(
            collect(&source, &[1]).unwrap(),
            (1..=7)
                .map(|id| vec![Value::Int32(id * 10)])
                .collect::<Vec<_>>()
        );
        let loaded = reads.get();
        assert_eq!(collect(&source, &[]).unwrap(), vec![vec![]; 7]);
        assert!(
            reads.get() > loaded,
            "progressive success must not populate a cache"
        );
        assert_eq!(
            source.scan("Items", &mut |_| Ok(())),
            Err(QueryError::StructureLimit)
        );
    }
}

#[test]
fn successful_progressive_scan_does_not_populate_ordinary_snapshot_or_projection_caches() {
    let input = watched(fixture::database());
    let reads = input.reads.clone();
    let fail = input.failed_page.clone();
    let source = NativeSource::new_with_snapshot_limits(
        input,
        SCOPE,
        budget(),
        &|| false,
        TableSnapshotLimits::new(3, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(collect(&source, &[0]).unwrap().len(), 3);
    fail.set(Some(12));
    assert_eq!(collect(&source, &[0]), Err(QueryError::Source));
    assert_eq!(
        source.scan_projected("Items", &[0], None, &mut |_| Ok(())),
        Err(QueryError::Source)
    );
    assert!(reads.get() > 0);
}

#[test]
fn late_read_and_count_failures_preserve_only_prior_callbacks_and_never_cache_suffixes() {
    for corrupt_count in [false, true] {
        let mut bytes = map_fixture::database(4, true, false);
        if corrupt_count {
            support::put32(&mut bytes, TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 5);
        }
        let input = watched(bytes);
        let fail = input.failed_page.clone();
        if !corrupt_count {
            fail.set(Some(map_fixture::FIRST_DATA_PAGE + 3));
        }
        let source = source(input, 1);
        let mut seen = Vec::new();
        assert_eq!(
            source.scan_projected_progressive("Items", &[0], &mut |row| {
                seen.push(row.get(0).unwrap().clone());
                Ok(())
            }),
            Err(QueryError::Source)
        );
        assert_eq!(seen.len(), if corrupt_count { 4 } else { 3 });
        assert_eq!(seen[0], Value::Int32(1));
        fail.set(None);
        if !corrupt_count {
            assert_eq!(collect(&source, &[0]).unwrap().len(), 4);
        }
    }
}

#[test]
fn callback_failure_stops_immediately_and_explicit_retry_starts_at_first_row() {
    let source = source(watched(fixture::database()), 2);
    let mut visits = 0;
    assert_eq!(
        source.scan_projected_progressive("Items", &[0], &mut |_| {
            visits += 1;
            Err(QueryError::Parameter)
        }),
        Err(QueryError::Parameter)
    );
    assert_eq!(visits, 1);
    assert_eq!(collect(&source, &[0]).unwrap()[0], vec![Value::Int32(1)]);
}

#[test]
fn callbacks_release_shared_budget_but_keep_native_cursor_exclusive() {
    let pool = RefCell::new(budget());
    let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
        watched(fixture::database()),
        SCOPE,
        &pool,
        &|| false,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap();
    let mut visits = 0;
    source
        .scan_projected_progressive("Items", &[0], &mut |_| {
            pool.try_borrow_mut().unwrap().charge(Resource::Steps, 1)?;
            assert_eq!(collect(&source, &[0]), Err(QueryError::Source));
            visits += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(visits, 3);
    assert_eq!(collect(&source, &[0]).unwrap().len(), 3);
}

#[test]
fn cancellation_and_cumulative_limits_stop_later_batches() {
    for cancel in [false, true] {
        let pool = RefCell::new(budget());
        let stopped = Cell::new(false);
        let cancelled = || stopped.get();
        let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            watched(fixture::database()),
            SCOPE,
            &pool,
            &cancelled,
            TableSnapshotLimits::new(1, 1).unwrap(),
        )
        .unwrap();
        let mut visits = 0;
        let result = source.scan_projected_progressive("Items", &[0], &mut |_| {
            visits += 1;
            if cancel {
                stopped.set(true);
            } else {
                let mut allowance = pool.borrow_mut();
                let remaining = allowance.remaining(Resource::Rows);
                allowance.charge(Resource::Rows, remaining)?;
            }
            Ok(())
        });
        assert!(if cancel {
            result == Err(QueryError::Cancelled)
        } else {
            matches!(result, Err(QueryError::Limit(_)))
        });
        assert_eq!(visits, 1);
    }
}

#[test]
fn bound_schema_changes_and_invalid_projections_fail_before_callback() {
    let input = watched(fixture::database());
    let changed = input.changed_schema.clone();
    let source = source(input, 1);
    source.columns("Items").unwrap();
    for indices in [&[2][..], &[0, 0], &[1, 0]] {
        assert_eq!(
            source.scan_projected_progressive("Items", indices, &mut |_| panic!(
                "invalid projection callback"
            )),
            Err(QueryError::Source)
        );
    }
    changed.set(true);
    assert_eq!(
        source.scan_projected_progressive("Items", &[0], &mut |_| panic!(
            "changed unselected schema callback"
        )),
        Err(QueryError::SchemaChanged)
    );
}

#[test]
fn explicit_indexes_advanced_modes_and_linked_tables_refuse_before_callbacks() {
    for mode in 0..4 {
        let source = source(watched(fixture::database()), 1);
        let source = match mode {
            0 => source.with_index("Items", "MissingIndex").unwrap(),
            1 => source.with_multivalues(),
            2 => source.with_collections(),
            _ => source.with_decoded_attachments(),
        };
        assert_eq!(
            source
                .scan_projected_progressive("Items", &[0], &mut |_| panic!("unsupported callback")),
            Err(QueryError::Unsupported)
        );
    }
    let source = source(
        watched(linked_fixture::database(
            6,
            Some("opaque"),
            Some("Items"),
            None,
        )),
        1,
    );
    assert_eq!(
        source.scan_projected_progressive("Linked", &[], &mut |_| panic!("linked callback")),
        Err(QueryError::Unsupported)
    );
}

#[test]
fn ordinary_projection_prunes_bad_payloads_and_unavailable_fields() {
    const USAGE_PAGE: u32 = 11;
    const DATA_PAGE: usize = 12;
    for unavailable in [false, true] {
        let mut bytes = fixture::database();
        bytes[TABLE_PAGE * PAGE_BYTES..(TABLE_PAGE + 1) * PAGE_BYTES].copy_from_slice(
            &fixture::definition(&[("Id", 4, 0, 4), ("BadDate", 8, 4, 8)], 3, USAGE_PAGE << 8),
        );
        if unavailable {
            bytes[TABLE_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES] = 0xff;
        }
        let rows = (1_i32..=3)
            .map(|id| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend(id.to_le_bytes());
                row.extend(f64::NAN.to_bits().to_le_bytes());
                row.push(3);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(TABLE_PAGE as u32, &rows));
        let source = source(watched(bytes), 1).with_auto_indexes();
        assert_eq!(
            collect(&source, &[0]).unwrap(),
            vec![
                vec![Value::Int32(1)],
                vec![Value::Int32(2)],
                vec![Value::Int32(3)],
            ]
        );
        if !unavailable {
            assert!(collect(&source, &[1]).is_err());
        }
    }
}

#[test]
fn empty_table_still_qualifies_metadata_and_checks_eof() {
    let source = source(watched(map_fixture::database(0, true, false)), 1);
    assert!(collect(&source, &[0]).unwrap().is_empty());
    assert!(collect(&source, &[]).unwrap().is_empty());
}

#[test]
fn progressive_scan_never_reuses_a_previous_complete_snapshot() {
    let input = watched(fixture::database());
    let fail = input.failed_page.clone();
    let source = NativeSource::new(input, SCOPE, budget(), &|| false).unwrap();
    source.scan("Items", &mut |_| Ok(())).unwrap();
    fail.set(Some(12));
    source.scan("Items", &mut |_| Ok(())).unwrap();
    assert_eq!(collect(&source, &[0]), Err(QueryError::Source));
}

#[path = "progressive_source/grouped.rs"]
mod grouped;
