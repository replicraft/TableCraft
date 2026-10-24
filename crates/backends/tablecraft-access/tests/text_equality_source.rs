use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    io::{self, Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_model::CatalogScope;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{CollationResolver, TextCollationId},
};
use tablecraft_sql::parse_query;

const INDEX_OFFSET: u64 = 740 * 4096;

struct Watching {
    bytes: Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
    forbid: bool,
}

impl NativeReadSource for Watching {}

impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.bytes.position() == INDEX_OFFSET {
            self.reads.set(self.reads.get() + 1);
            if self.forbid {
                return Err(io::Error::other("unexpected index traversal"));
            }
        }
        self.bytes.read(bytes)
    }
}

impl Seek for Watching {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 100_000_000)
}

fn source(reads: Rc<Cell<usize>>, rows: u32, forbid: bool) -> NativeSource<'static, Watching> {
    NativeSource::new_with_snapshot_limits(
        Watching {
            bytes: Cursor::new(
                include_bytes!("../../../../docs/examples/Access_Example_VBA.accdb").to_vec(),
            ),
            reads,
            forbid,
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
        TableSnapshotLimits::new(rows, 1).unwrap(),
    )
    .unwrap()
    .with_auto_indexes()
}

struct UnqualifiedOrdinal;

impl Collation for UnqualifiedOrdinal {
    fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(a.cmp(b))
    }
}

impl CollationResolver for UnqualifiedOrdinal {
    fn resolve(&self, _policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        Ok(self)
    }
}

#[test]
fn native_equality_requires_actual_comparer_agreement_and_keeps_partial_caches_separate() {
    let reads = Rc::new(Cell::new(0));
    let pool = RefCell::new(budget());
    let (source, resolver) = source(reads.clone(), 1, false)
        .with_database_collation_resolver(&pool)
        .unwrap();
    let sql = parse_query(
        "SELECT TAG_GRP_ID FROM TAG_GRP_TB WHERE TAG_GRP_NME='Office Objects'",
        Default::default(),
    )
    .unwrap();
    let before = reads.get();
    let plan = prepare(sql.query(), &source).unwrap();
    assert_eq!(reads.get(), before);

    // Identical opaque field IDs do not qualify a different actual comparer.
    let refused = plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation_resolver: Some(&UnqualifiedOrdinal),
            ..Default::default()
        },
    );
    assert_eq!(refused.err(), Some(QueryError::StructureLimit));
    assert_eq!(reads.get(), before);

    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(9)]]);
    assert!(reads.get() > before);

    let all = parse_query("SELECT Count(*) FROM TAG_GRP_TB", Default::default()).unwrap();
    assert_eq!(
        prepare(all.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(QueryError::StructureLimit)
    );
}

#[test]
fn unsafe_compound_filters_never_issue_partial_text_hints() {
    let reads = Rc::new(Cell::new(0));
    let pool = RefCell::new(budget());
    let (source, resolver) = source(reads.clone(), 16384, true)
        .with_database_collation_resolver(&pool)
        .unwrap();
    let sql = parse_query(
        "SELECT TAG_GRP_ID FROM TAG_GRP_TB WHERE TAG_GRP_NME='Office Objects' AND 1/0=0",
        Default::default(),
    )
    .unwrap();
    let result = prepare(sql.query(), &source).unwrap().execute(
        &source,
        &[],
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(
        result.err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(reads.get(), 0);
}
