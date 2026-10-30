use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access as support;

const PAGE_BYTES: usize = 4096;
const CHILD_PAGE: usize = 22;
const PARENT_DATA_PAGE: usize = 12;
fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1000000)
}
fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
struct Watching {
    data: Cursor<Vec<u8>>,
    child_reads: Rc<Cell<usize>>,
    forbid_children: Rc<Cell<bool>>,
    forbid_parent: Rc<Cell<bool>>,
}
impl tablecraft_ace::NativeReadSource for Watching {}
impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let position = self.data.position() as usize;
        if position / PAGE_BYTES == PARENT_DATA_PAGE {
            assert!(!self.forbid_parent.get(), "binding read parent records");
        }
        if position / PAGE_BYTES == CHILD_PAGE {
            self.child_reads.set(self.child_reads.get() + 1);
            assert!(
                !self.forbid_children.get(),
                "binding/scalar query read child records"
            );
        }
        self.data.read(bytes)
    }
}
impl Seek for Watching {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        self.data.seek(p)
    }
}
#[test]
fn explicit_members_expand_native_parent_order_and_cache_complete_collections() {
    for code in [2, 5, 6] {
        let mut bytes = links_fixture::populated();
        bytes[20] = code;
        let child_reads = Rc::new(Cell::new(0));
        let forbid = Rc::new(Cell::new(true));
        let forbid_parent = Rc::new(Cell::new(true));
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(bytes),
                child_reads: child_reads.clone(),
                forbid_children: forbid.clone(),
                forbid_parent: forbid_parent.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();
        let parsed = parse_query(
            "SELECT Id,Score.Value AS V FROM Items ORDER BY Id,V",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(child_reads.get(), 0);
        forbid_parent.set(false);
        assert_eq!(source.multivalue_columns("Items").unwrap().len(), 1);
        assert_eq!(
            execute(&source, "SELECT Count(*) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
        assert_eq!(child_reads.get(), 0);
        forbid.set(false);
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [
                vec![Value::Int32(1), Value::Int32(100)],
                vec![Value::Int32(1), Value::Int32(101)],
                vec![Value::Int32(2), Value::Int32(200)],
                vec![Value::Int32(3), Value::Null]
            ]
        );
        let reads = child_reads.get();
        assert!(reads > 0);
        forbid.set(true);
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Count(Score.Value) FROM Items GROUP BY Id ORDER BY Id"
            )
            .unwrap(),
            [
                vec![Value::Int32(1), Value::Int64(2)],
                vec![Value::Int32(2), Value::Int64(1)],
                vec![Value::Int32(3), Value::Int64(0)]
            ]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Score.Value AS V FROM Items WHERE Score.Value=100 ORDER BY V"
            )
            .unwrap(),
            [vec![Value::Int32(100)]]
        );
        assert_eq!(
            execute(&source, "SELECT Count(*) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
        assert_eq!(child_reads.get(), reads);
        assert_eq!(
            execute(
                &source,
                "SELECT Id FROM Items WHERE Score.Value=100 AND Score.Value=101"
            )
            .unwrap(),
            [vec![Value::Int32(1)]]
        );
        assert_eq!(
            execute(
                &source,
                "SELECT Count(*) FROM Items WHERE Score.Value IN (100,101)"
            )
            .unwrap(),
            [vec![Value::Int64(1)]]
        );
        assert_eq!(child_reads.get(), reads);
    }
}
#[test]
fn default_sources_and_selected_indexes_do_not_silently_expand_or_fall_back() {
    let bytes = links_fixture::populated();
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    assert!(source.multivalue_columns("Items").unwrap().is_empty());
    assert_eq!(
        execute(&source, "SELECT Score.Value FROM Items").err(),
        Some(QueryError::UnknownName)
    );
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_multivalues()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score.Value FROM Items").err(),
        Some(QueryError::UnknownName)
    );
}
#[test]
fn malformed_children_limits_and_cancellation_fail_without_parent_placeholders() {
    let mut bytes = links_fixture::populated();
    bytes[CHILD_PAGE * PAGE_BYTES] = 0;
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_multivalues();
    assert_eq!(
        execute(&source, "SELECT Count(*) FROM Items").unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert!(execute(&source, "SELECT Score.Value FROM Items").is_err());
    let cancel = Cell::new(false);
    let cancelled = || cancel.get();
    let source = NativeSource::new(
        Cursor::new(links_fixture::populated()),
        CatalogScope::new(1),
        budget(),
        &cancelled,
    )
    .unwrap()
    .with_multivalues();
    let parsed = parse_query("SELECT Score.Value FROM Items", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    cancel.set(true);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Cancelled)
    );
    let source = NativeSource::new(
        Cursor::new(links_fixture::populated()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: tablecraft_query::QueryLimits {
                    rows: 2,
                    ..Default::default()
                },
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod attachment_fixture;
#[test]
fn attachment_metadata_aligns_and_reuses_one_complete_snapshot_without_payload_decoding() {
    for code in [2, 5, 6] {
        let mut bytes = attachment_fixture::database();
        bytes[20] = code;
        let reads = Rc::new(Cell::new(0));
        let forbid = Rc::new(Cell::new(true));
        let parent = Rc::new(Cell::new(true));
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(bytes),
                child_reads: reads.clone(),
                forbid_children: forbid.clone(),
                forbid_parent: parent.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap()
        .with_collections();
        let parsed = parse_query(
            "SELECT Id,Score.FileName,Score.FileType FROM Items",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(reads.get(), 0);
        forbid.set(false);
        parent.set(false);
        let expected = [
            vec![
                Value::Int32(1),
                Value::Text("../é🦀\0.bin".into()),
                Value::Text("field/type".into()),
            ],
            vec![Value::Int32(1), Value::Null, Value::Null],
            vec![
                Value::Int32(2),
                Value::Text("".into()),
                Value::Text("".into()),
            ],
            vec![Value::Int32(3), Value::Null, Value::Null],
        ];
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            expected
        );
        assert!(reads.get() > 0);
        let count = reads.get();
        forbid.set(true);
        assert_eq!(
            execute(
                &source,
                "SELECT Id,Count(Score.FileName) FROM Items GROUP BY Id ORDER BY Id"
            )
            .unwrap(),
            [
                vec![Value::Int32(1), Value::Int64(1)],
                vec![Value::Int32(2), Value::Int64(1)],
                vec![Value::Int32(3), Value::Int64(0)]
            ]
        );
        assert_eq!(
            execute(&source, "SELECT Id FROM Items WHERE Score.FileFlags<0").unwrap(),
            [vec![Value::Int32(1)], vec![Value::Int32(2)]]
        );
        assert_eq!(reads.get(), count);
        forbid.set(false);
        assert!(execute(&source, "SELECT Score.FileData FROM Items").is_ok());
        forbid.set(true);
        assert_eq!(
            execute(&source, "SELECT Count(*) FROM Items").unwrap(),
            [vec![Value::Int64(3)]]
        );
    }
}
#[test]
fn attachment_opt_in_index_and_corrupt_child_boundaries_are_explicit() {
    let bytes = attachment_fixture::database();
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    assert!(source.collection_members("Items").unwrap().is_empty());
    assert!(execute(&source, "SELECT Score.FileName FROM Items").is_err());
    let source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_collections()
    .with_index("Items", "Missing")
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Score.FileName FROM Items").err(),
        Some(QueryError::UnknownName)
    );
    let mut bytes = bytes;
    bytes[CHILD_PAGE * PAGE_BYTES] = 0;
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_collections();
    assert_eq!(source.collection_members("Items").unwrap().len(), 6);
    assert_eq!(
        execute(&source, "SELECT Score.FileName FROM Items").err(),
        Some(QueryError::Source)
    );
}

#[test]
fn statistical_aggregates_use_stored_collection_values_and_keep_empty_parent_rules() {
    use tablecraft_core::values::Double;
    for code in [2, 5, 6] {
        let mut bytes = links_fixture::populated();
        bytes[20] = code;
        let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
            false
        })
        .unwrap()
        .with_multivalues();
        assert_eq!(execute(&source,"SELECT Id,Var(Score.Value),VarP(Score.Value),StDevP(Score.Value) FROM Items GROUP BY Id ORDER BY Id").unwrap(),[
            vec![Value::Int32(1),Value::Double(Double::from_float(0.5)),Value::Double(Double::from_float(0.25)),Value::Double(Double::from_float(0.5))],
            vec![Value::Int32(2),Value::Null,Value::Null,Value::Double(Double::from_float(0.0))],
            vec![Value::Int32(3),Value::Null,Value::Null,Value::Null],
        ]);
    }
}

#[test]
fn batched_collection_failures_publish_no_cache_and_explicit_retry_completes_once() {
    struct FailsOnce {
        input: Cursor<Vec<u8>>,
        fail: Rc<Cell<bool>>,
        child_reads: Rc<Cell<usize>>,
    }

    impl tablecraft_ace::NativeReadSource for FailsOnce {}

    impl Read for FailsOnce {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            if self.input.position() == (CHILD_PAGE * PAGE_BYTES) as u64 {
                self.child_reads.set(self.child_reads.get() + 1);
                if self.fail.replace(false) {
                    return Err(std::io::ErrorKind::PermissionDenied.into());
                }
            }
            self.input.read(bytes)
        }
    }

    impl Seek for FailsOnce {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.input.seek(position)
        }
    }

    let fail = Rc::new(Cell::new(true));
    let child_reads = Rc::new(Cell::new(0));
    let source = NativeSource::new_with_snapshot_limits(
        FailsOnce {
            input: Cursor::new(links_fixture::populated()),
            fail: fail.clone(),
            child_reads: child_reads.clone(),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
        tablecraft_ace::TableSnapshotLimits::new(3, 1).unwrap(),
    )
    .unwrap()
    .with_multivalues();
    let sql = "SELECT Id,Count(Score.Value) FROM Items GROUP BY Id ORDER BY Id";
    assert_eq!(execute(&source, sql), Err(QueryError::Source));
    assert_eq!(child_reads.get(), 1);

    let expected = vec![
        vec![Value::Int32(1), Value::Int64(2)],
        vec![Value::Int32(2), Value::Int64(1)],
        vec![Value::Int32(3), Value::Int64(0)],
    ];
    assert_eq!(execute(&source, sql).unwrap(), expected);
    let completed_reads = child_reads.get();
    assert!(completed_reads > 1);

    fail.set(true);
    assert_eq!(execute(&source, sql).unwrap(), expected);
    assert_eq!(child_reads.get(), completed_reads);
    assert!(fail.get(), "complete caches avoid another child read");
}
