use tablecraft_testkit::access::mutation_fixture as base;
#[path = "../../../engines/tablecraft-ace/tests/guid_indexes/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, Source, prepare, source::ScanConstraint,
};
use tablecraft_sql::parse_query;

struct Reads {
    source: Cursor<Vec<u8>>,
    data: Rc<Cell<usize>>,
    index: Rc<Cell<usize>>,
}
impl tablecraft_ace::NativeReadSource for Reads {}
impl Read for Reads {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.source.position() == 12 * 4096 {
            self.data.set(self.data.get() + 1);
        }
        if self.source.position() == 14 * 4096 {
            self.index.set(self.index.get() + 1);
        }
        self.source.read(bytes)
    }
}
impl Seek for Reads {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.source.seek(position)
    }
}
fn execute(sql: &str, source: &dyn Source) -> Result<QueryResult, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source)?.execute(source, &[], ExecutionOptions::default())
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn bytes(code: u8, descending: bool) -> Vec<u8> {
    fixture::database(
        code,
        descending,
        &[
            None,
            Some([0; 16]),
            Some([0x80; 16]),
            Some([0x80; 16]),
            Some([0xff; 16]),
            None,
        ],
        0,
    )
}

#[test]
fn guid_seeks_bind_metadata_only_and_do_not_cache_partial_tables() {
    for code in [5, 6] {
        for descending in [false, true] {
            let data = Rc::new(Cell::new(0));
            let index = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Reads {
                    source: Cursor::new(bytes(code, descending)),
                    data: data.clone(),
                    index: index.clone(),
                },
                CatalogScope::new(7),
                budget(),
                &|| false,
            )
            .unwrap()
            .with_index("Items", "Compound")
            .unwrap();
            let parsed = parse_query(
                "PARAMETERS p GUID; SELECT Id, Score FROM Items WHERE Score=p ORDER BY Id",
                Default::default(),
            )
            .unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!((data.get(), index.get()), (0, 0));
            for (value, ids) in [
                (Value::Guid(Guid::from_bytes([0x80; 16])), vec![3, 4]),
                (Value::Guid(Guid::from_bytes([0; 16])), vec![2]),
                (Value::Guid(Guid::from_bytes([1; 16])), vec![]),
                (Value::Null, vec![]),
            ] {
                let result = plan
                    .execute(
                        &source,
                        &[("p", value.clone())],
                        ExecutionOptions::default(),
                    )
                    .unwrap();
                let expected = ids
                    .into_iter()
                    .map(|id| vec![Value::Int32(id), value.clone()])
                    .collect::<Vec<_>>();
                assert_eq!(result.rows(), expected);
            }
            // The NULL parameter declines the hint and supplies a complete cache.
            let complete = (data.get(), index.get());
            assert_eq!(
                execute(
                    "SELECT Id FROM Items WHERE Score IS NULL ORDER BY Id",
                    &source
                )
                .unwrap()
                .rows(),
                &[vec![Value::Int32(1)], vec![Value::Int32(6)]]
            );
            assert_eq!(
                execute("SELECT Count(*) FROM Items", &source)
                    .unwrap()
                    .rows(),
                &[vec![Value::Int64(6)]]
            );
            assert_eq!((data.get(), index.get()), complete);
        }
    }
}

#[test]
fn guid_misses_skip_user_records_and_later_complete_scans_keep_every_row() {
    let data = Rc::new(Cell::new(0));
    let index = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Reads {
            source: Cursor::new(bytes(6, false)),
            data: data.clone(),
            index: index.clone(),
        },
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_index("Items", "Compound")
    .unwrap();
    assert!(
        execute(
            "SELECT Id FROM Items WHERE Score={GUID {01010101-0101-0101-0101-010101010101}}",
            &source
        )
        .unwrap()
        .rows()
        .is_empty()
    );
    assert_eq!((data.get(), index.get()), (0, 1));
    assert_eq!(
        execute("SELECT Id FROM Items WHERE Score IS NULL", &source)
            .unwrap()
            .rows()
            .len(),
        2
    );
    assert_eq!((data.get(), index.get()), (1, 2));
    assert_eq!(
        execute("SELECT Id, Score FROM Items", &source)
            .unwrap()
            .rows()
            .len(),
        6
    );
    assert_eq!((data.get(), index.get()), (2, 3));
    execute(
        "SELECT Id FROM Items WHERE Score={GUID {00000000-0000-0000-0000-000000000000}}",
        &source,
    )
    .unwrap();
    assert_eq!((data.get(), index.get()), (2, 3));
}

#[test]
fn guid_hints_preserve_mandatory_index_and_selected_corruption_errors() {
    for index in ["missing", "Compound"] {
        let mut input = bytes(6, false);
        if index == "Compound" {
            input[14 * 4096] = 0xff;
        }
        let source = NativeSource::new(Cursor::new(input), CatalogScope::new(7), budget(), &|| {
            false
        })
        .unwrap()
        .with_index("Items", index)
        .unwrap();
        assert!(
            execute(
                "SELECT Id FROM Items WHERE Score={GUID {00000000-0000-0000-0000-000000000000}}",
                &source
            )
            .is_err()
        );
        assert!(
            source
                .scan_constrained(
                    "Items",
                    ScanConstraint::Int32 {
                        column: 1,
                        lower: std::ops::Bound::Unbounded,
                        upper: std::ops::Bound::Unbounded
                    },
                    &mut |_| Ok(())
                )
                .is_err()
        );
    }
}

#[test]
fn guid_mixed_schema_maps_dense_ordinals_without_reading_opaque_fields() {
    for descending in [false, true] {
        let mut input = bytes(6, descending);
        input[10 * 4096 + 75] = 0xff; // Independent fixture: first field has an unknown native type.
        let source = NativeSource::new(Cursor::new(input), CatalogScope::new(7), budget(), &|| {
            false
        })
        .unwrap()
        .with_index("Items", "Compound")
        .unwrap();
        assert_eq!(source.columns("Items").unwrap().len(), 1);
        assert_eq!(
            execute(
                "SELECT Score FROM Items WHERE Score={GUID {80808080-8080-8080-8080-808080808080}}",
                &source
            )
            .unwrap()
            .rows(),
            &[
                vec![Value::Guid(Guid::from_bytes([0x80; 16]))],
                vec![Value::Guid(Guid::from_bytes([0x80; 16]))]
            ]
        );
        assert_eq!(
            execute("SELECT Score FROM Items WHERE Score IS NULL", &source)
                .unwrap()
                .rows(),
            &[vec![Value::Null], vec![Value::Null]]
        );
        assert_eq!(
            execute("SELECT Score FROM Items", &source)
                .unwrap()
                .rows()
                .len(),
            6
        );
        assert!(execute("SELECT Id FROM Items", &source).is_err());
    }
}

#[test]
fn guid_selected_visitors_cannot_silence_errors_or_cancellation() {
    let source = NativeSource::new(
        Cursor::new(bytes(6, false)),
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_index("Items", "Compound")
    .unwrap();
    let constraint = ScanConstraint::Guid {
        column: 1,
        value: Guid::from_bytes([0x80; 16]),
    };
    let mut visits = 0;
    assert_eq!(
        source.scan_constrained("Items", constraint, &mut |_| {
            visits += 1;
            Err(QueryError::Unsupported)
        }),
        Err(QueryError::Unsupported)
    );
    assert_eq!(visits, 1);

    let cancelled = Cell::new(false);
    let check = || cancelled.get();
    let source = NativeSource::new(
        Cursor::new(bytes(6, false)),
        CatalogScope::new(7),
        budget(),
        &check,
    )
    .unwrap()
    .with_index("Items", "Compound")
    .unwrap();
    assert_eq!(
        source.scan_constrained("Items", constraint, &mut |_| {
            cancelled.set(true);
            Ok(())
        }),
        Err(QueryError::Cancelled)
    );
}
