#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
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
        Some(QueryError::Unsupported)
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
