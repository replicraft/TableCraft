//! Original indexed stored caches, independent of expression evaluation.
use base as fixture;
use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "required_index_calculations/fixture.rs"]
mod indexed_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachment_modern/fixture.rs"]
mod modern_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod old_attachment;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, NativeReadSource,
    read_stored_table_index_with_references,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

fn expected(descending: bool) -> Vec<Vec<Value>> {
    let order = if descending {
        [0, 3, 1, 2]
    } else {
        [1, 2, 3, 0]
    };
    order
        .into_iter()
        .map(|slot| {
            vec![
                Value::Int32(indexed_fixture::IDS[slot]),
                indexed_fixture::CACHES[slot].map_or(Value::Null, Value::Int32),
            ]
        })
        .collect()
}

struct Watching {
    bytes: Cursor<Vec<u8>>,
    blocked: Rc<Cell<Option<usize>>>,
    reads: Rc<Cell<usize>>,
}
impl NativeReadSource for Watching {}
impl Read for Watching {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        let page = self.bytes.position() as usize / 4096;
        if self.blocked.get() == Some(page) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        if matches!(page, 12 | 22 | 26 | 27) {
            self.reads.set(self.reads.get() + 1);
        }
        self.bytes.read(target)
    }
}
impl Seek for Watching {
    fn seek(&mut self, target: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(target)
    }
}

#[test]
fn stale_stored_values_and_nulls_follow_ordinary_index_rows_without_formula_execution() {
    for code in [5, 6] {
        for descending in [false, true] {
            for forwarded in [false, true] {
                let bytes = indexed_fixture::plain(code, descending, forwarded);
                let direct = read_stored_table_index_with_references(
                    &mut Cursor::new(bytes.clone()),
                    SCOPE,
                    "Items",
                    "IdOrder",
                    IndexReadRequest {
                        columns: Some(&["Score"]),
                        selection: IndexProjectionSelection::All,
                        maximum_rows: 10,
                    },
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let slots = if descending {
                    [0, 3, 1, 2]
                } else {
                    [1, 2, 3, 0]
                };
                assert_eq!(
                    direct
                        .row_references()
                        .iter()
                        .map(|root| (root.page_number(), root.slot_number()))
                        .collect::<Vec<_>>(),
                    slots.map(|slot| (12, slot))
                );
                assert_eq!(
                    direct.snapshot().rows(),
                    expected(descending)
                        .into_iter()
                        .map(|row| vec![row[1].clone()])
                        .collect::<Vec<_>>()
                );
                let source = NativeSource::new(Cursor::new(bytes), SCOPE, budget(), &|| false)
                    .unwrap()
                    .with_stored_calculations()
                    .unwrap()
                    .with_index("Items", "IdOrder")
                    .unwrap();
                assert_eq!(
                    execute(&source, "SELECT Id,Score FROM Items").unwrap(),
                    expected(descending)
                );
                assert_eq!(
                    execute(&source, "SELECT Score FROM Items").unwrap(),
                    expected(descending)
                        .into_iter()
                        .map(|row| vec![row[1].clone()])
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn constrained_then_complete_stored_reads_never_cache_a_selected_prefix() {
    let blocked = Rc::new(Cell::new(None));
    let reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(indexed_fixture::plain(6, false, true)),
            blocked: blocked.clone(),
            reads: reads.clone(),
        },
        SCOPE,
        budget(),
        &|| false,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_index("Items", "IdOrder")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score FROM Items WHERE Id=10").unwrap(),
        [vec![Value::Null], vec![Value::Int32(41)]]
    );
    assert_eq!(
        execute(&source, "SELECT Id,Score FROM Items").unwrap(),
        expected(false)
    );
    let count = reads.get();
    assert!(count > 0);
    blocked.set(Some(12));
    assert_eq!(
        execute(&source, "SELECT Score FROM Items").unwrap(),
        expected(false)
            .into_iter()
            .map(|row| vec![row[1].clone()])
            .collect::<Vec<_>>()
    );
    assert_eq!(reads.get(), count);
}

#[test]
fn hidden_ordinary_key_is_checked_and_unselected_bad_cache_is_pruned() {
    let mut bytes = indexed_fixture::plain(6, false, true);
    let start = indexed_fixture::row_start(&bytes, 26, 0);
    bytes[start + 2..start + 6].copy_from_slice(&11_i32.to_le_bytes());
    let source = NativeSource::new(Cursor::new(bytes), SCOPE, budget(), &|| false)
        .unwrap()
        .with_stored_calculations()
        .unwrap()
        .with_index("Items", "IdOrder")
        .unwrap();
    let callbacks = Cell::new(0);
    assert!(
        source
            .scan_projected("Items", &[1], None, &mut |_| {
                callbacks.set(callbacks.get() + 1);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(callbacks.get(), 0);

    let mut bytes = indexed_fixture::plain(6, false, false);
    let start = indexed_fixture::row_start(&bytes, 12, 3);
    bytes[start + 6] = 1; // Unsupported wrapper tag in a late selected cache.
    let source = NativeSource::new(Cursor::new(bytes), SCOPE, budget(), &|| false)
        .unwrap()
        .with_stored_calculations()
        .unwrap()
        .with_index("Items", "IdOrder")
        .unwrap();
    assert_eq!(
        execute(&source, "SELECT Id FROM Items").unwrap(),
        [
            vec![Value::Int32(10)],
            vec![Value::Int32(10)],
            vec![Value::Int32(20)],
            vec![Value::Int32(30)]
        ]
    );
    for _ in 0..2 {
        assert!(
            source
                .scan_projected("Items", &[1], None, &mut |_| {
                    callbacks.set(callbacks.get() + 1);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(callbacks.get(), 0);
    }
}

#[test]
fn cached_scalar_and_attachment_members_share_logical_root_alignment() {
    for code in [5, 6] {
        for descending in [false, true] {
            let source = NativeSource::new(
                Cursor::new(indexed_fixture::mixed(code, descending, true)),
                SCOPE,
                budget(),
                &|| false,
            )
            .unwrap()
            .with_stored_calculations()
            .unwrap()
            .with_collections()
            .with_index("Items", "IdOrder")
            .unwrap();
            let declarations = source.collection_members("Items").unwrap();
            let member = declarations
                .iter()
                .position(|member| member.column().name() == "FileName")
                .unwrap();
            let mut observed = Vec::new();
            source
                .scan_collection_members("Items", &[0, 1], &[], &[member], &mut |row, fields| {
                    observed.push((
                        row.get(0).unwrap().clone(),
                        row.get(1).unwrap().clone(),
                        (0..fields[0].len())
                            .map(|index| fields[0].get(index).unwrap().clone())
                            .collect::<Vec<_>>(),
                    ));
                    Ok(())
                })
                .unwrap();
            let order = if descending {
                [0, 3, 1, 2]
            } else {
                [1, 2, 3, 0]
            };
            let names = [
                vec![Value::Text("synthetic.bin".into()), Value::Null],
                vec![],
                vec![Value::Text(String::new())],
                vec![],
            ];
            let expected = order
                .into_iter()
                .map(|slot| {
                    (
                        Value::Int32(indexed_fixture::IDS[slot]),
                        indexed_fixture::CACHES[slot].map_or(Value::Null, Value::Int32),
                        names[slot].clone(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(observed, expected);
            assert_eq!(
                execute(&source, "SELECT Id,Cached,Score.FileName FROM Items")
                    .unwrap()
                    .len(),
                5
            );
        }
    }
}

#[test]
fn indexed_cached_metadata_defers_file_data_until_explicit_selection() {
    let mut bytes = indexed_fixture::mixed(6, false, true);
    let mut descriptor = (0x40000000 | old_attachment::PAYLOAD.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((27_u32 << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    modern_fixture::install_rows(
        &mut bytes,
        &[
            modern_fixture::row(10, 1, 0, &descriptor),
            modern_fixture::row(10, 2, 1, &[]),
            modern_fixture::row(20, 3, 2, &[]),
        ],
    );
    bytes.resize(28 * 4096, 0);
    let mut page = support::data_page(0, &[(0, old_attachment::PAYLOAD.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    bytes[27 * 4096..28 * 4096].copy_from_slice(&page);
    let blocked = Rc::new(Cell::new(Some(27)));
    let source = NativeSource::new(
        Watching {
            bytes: Cursor::new(bytes),
            blocked: blocked.clone(),
            reads: Rc::new(Cell::new(0)),
        },
        SCOPE,
        budget(),
        &|| false,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_collections()
    .with_index("Items", "IdOrder")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Id,Cached,Score.FileName FROM Items")
            .unwrap()
            .len(),
        5
    );
    assert!(execute(&source, "SELECT Cached,Score.FileData FROM Items").is_err());
    blocked.set(None);
    assert_eq!(
        execute(&source, "SELECT Count(Score.FileData) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
}

#[test]
fn selected_cached_index_reads_respect_cancellation_and_resource_limits() {
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let source = NativeSource::new(
        Cursor::new(indexed_fixture::plain(6, false, false)),
        SCOPE,
        budget(),
        &cancel,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_index("Items", "IdOrder")
    .unwrap();
    source.columns("Items").unwrap();
    cancelled.set(true);
    assert_eq!(
        execute(&source, "SELECT Score FROM Items").err(),
        Some(QueryError::Cancelled)
    );
    assert!(
        NativeSource::new(
            Cursor::new(indexed_fixture::plain(6, false, false)),
            SCOPE,
            ResourceBudget::new(1, 1, 1),
            &|| false
        )
        .is_err()
    );
}
