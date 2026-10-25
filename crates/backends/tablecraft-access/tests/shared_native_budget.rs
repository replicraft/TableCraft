//! Explicit host routing over two stable copies, not implicit linked-file opening.
use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_access::{NativeSavedReadError, NativeSource};
use tablecraft_ace::{InspectError, NativeReadSource};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::comparison::Collation;
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_schema::Column;
use tablecraft_sql::parse_query;

const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_STEPS: u64 = 1_000_000;
const MAX_ROWS: u64 = 16_384;

fn copy() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}

fn allowance(steps: u64) -> ResourceBudget {
    ResourceBudget::new(MAX_BYTES, MAX_ROWS, steps)
}

fn open<'a>(
    pool: &'a RefCell<ResourceBudget>,
    cancelled: &'a dyn Fn() -> bool,
) -> NativeSource<'a, Cursor<Vec<u8>>> {
    open_with_scope(pool, CatalogScope::new(7), cancelled)
}

fn open_with_scope<'a>(
    pool: &'a RefCell<ResourceBudget>,
    scope: CatalogScope,
    cancelled: &'a dyn Fn() -> bool,
) -> NativeSource<'a, Cursor<Vec<u8>>> {
    NativeSource::new_with_shared_budget(Cursor::new(copy()), scope, pool, cancelled).unwrap()
}

fn remaining(pool: &RefCell<ResourceBudget>) -> [u64; 3] {
    let budget = pool.borrow();
    [
        budget.remaining(Resource::Bytes),
        budget.remaining(Resource::Rows),
        budget.remaining(Resource::Steps),
    ]
}

fn read(source: &impl Source) -> Result<usize, QueryError> {
    let mut rows = 0;
    source.scan("ROULETTE_TB", &mut |_| {
        rows += 1;
        Ok(())
    })?;
    Ok(rows)
}

struct Routed<'s, 'a> {
    left: &'s NativeSource<'a, Cursor<Vec<u8>>>,
    right: &'s NativeSource<'a, Cursor<Vec<u8>>>,
}

impl Routed<'_, '_> {
    fn select(&self, table: &str) -> Result<&NativeSource<'_, Cursor<Vec<u8>>>, QueryError> {
        match table {
            "LeftTable" => Ok(self.left),
            "RightTable" => Ok(self.right),
            _ => Err(QueryError::UnknownName),
        }
    }
}

impl Source for Routed<'_, '_> {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.select(table)?.columns("ROULETTE_TB")
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.select(table)?.scan("ROULETTE_TB", visitor)
    }
}

fn join(source: &impl Source) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(
        "SELECT Count(*) FROM LeftTable AS L INNER JOIN RightTable AS R ON L.ORDR_NBR=R.ORDR_NBR",
        Default::default(),
    )
    .unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn two_individually_fitting_sources_exhaust_one_host_routed_join_pool() {
    let measure = RefCell::new(allowance(MAX_STEPS));
    let source = open(&measure, &|| false);
    source.columns("ROULETTE_TB").unwrap();
    let metadata = MAX_STEPS - measure.borrow().remaining(Resource::Steps);
    assert_eq!(read(&source), Ok(38));
    let complete = MAX_STEPS - measure.borrow().remaining(Resource::Steps);
    assert!(complete > metadata);
    let steps = metadata + complete;

    let left_pool = RefCell::new(allowance(steps));
    let right_pool = RefCell::new(allowance(steps));
    let left = open(&left_pool, &|| false);
    let right = open_with_scope(&right_pool, CatalogScope::new(8), &|| false);
    assert_eq!(
        join(&Routed {
            left: &left,
            right: &right
        })
        .unwrap(),
        [vec![Value::Int64(38)]]
    );

    let shared = RefCell::new(allowance(steps));
    let left = open(&shared, &|| false);
    let right = open_with_scope(&shared, CatalogScope::new(8), &|| false);
    left.columns("ROULETTE_TB").unwrap();
    right.columns("ROULETTE_TB").unwrap();
    assert!(matches!(
        join(&Routed { left: &left, right: &right }),
        Err(QueryError::Limit(limit)) if limit.resource == Resource::Steps
    ));
}

#[test]
fn cached_reuse_never_refunds_or_refills_the_shared_native_pool() {
    let pool = RefCell::new(allowance(MAX_STEPS));
    let first = open(&pool, &|| false);
    let second = open_with_scope(&pool, CatalogScope::new(8), &|| false);
    assert_eq!(read(&first), Ok(38));
    let loaded = remaining(&pool);
    assert_eq!(read(&first), Ok(38));
    first.columns("ROULETTE_TB").unwrap();
    assert_eq!(remaining(&pool), loaded);

    let steps = pool.borrow().remaining(Resource::Steps);
    pool.borrow_mut().charge(Resource::Steps, steps).unwrap();
    let exhausted = remaining(&pool);
    // Zero-I/O views keep their existing accounting; query work has its own quota.
    assert_eq!(read(&first), Ok(38));
    assert_eq!(remaining(&pool), exhausted);
    assert!(matches!(
        read(&second),
        Err(QueryError::Limit(limit)) if limit.resource == Resource::Steps
    ));
    assert!(remaining(&pool)[0] <= exhausted[0]);
    assert_eq!(remaining(&pool)[2], 0);
}

#[test]
fn setup_and_uncached_read_borrow_collisions_are_typed_not_panics() {
    let pool = RefCell::new(allowance(MAX_STEPS));
    let source = open(&pool, &|| false);
    let held = pool.borrow_mut();
    assert!(matches!(
        NativeSource::new_with_shared_budget(
            Cursor::new(copy()),
            CatalogScope::new(8),
            &pool,
            &|| false,
        ),
        Err(InspectError::Unsupported)
    ));
    assert_eq!(
        source.columns("ROULETTE_TB").err(),
        Some(QueryError::Source)
    );
    assert_eq!(read(&source), Err(QueryError::Source));
    assert!(matches!(
        source.with_stored_calculations(),
        Err(InspectError::Unsupported)
    ));
    drop(held);

    let source = open(&pool, &|| false);
    let held = pool.borrow();
    assert!(matches!(
        source.with_index("ROULETTE_TB", "Index"),
        Err(InspectError::Unsupported)
    ));
    drop(held);

    let source = open(&pool, &|| false);
    let held = pool.borrow_mut();
    assert!(matches!(
        source.with_database_collation(allowance(MAX_STEPS)),
        Err(InspectError::Unsupported)
    ));
    drop(held);

    let mut source = open(&pool, &|| false);
    let held = pool.borrow_mut();
    for select_only in [false, true] {
        let result = if select_only {
            source.read_saved_select("Missing", Default::default())
        } else {
            source.read_saved_query("Missing", Default::default())
        };
        assert!(matches!(
            result,
            Err(NativeSavedReadError::Native(InspectError::Unsupported))
        ));
    }
    drop(held);
    assert_eq!(read(&source), Ok(38));
}

struct Failing<'a> {
    cursor: Cursor<Vec<u8>>,
    fail: &'a Cell<bool>,
}

impl NativeReadSource for Failing<'_> {}

impl Read for Failing<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.fail.get() {
            return Err(std::io::Error::other("injected source failure"));
        }
        self.cursor.read(bytes)
    }
}

impl Seek for Failing<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}

#[test]
fn cancelled_and_failed_reads_keep_consumption_and_publish_no_partial_cache() {
    let pool = RefCell::new(allowance(MAX_STEPS));
    let checks = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() >= stop.get()
    };
    let source = open(&pool, &cancelled);
    source.columns("ROULETTE_TB").unwrap();
    let before = remaining(&pool);
    stop.set(checks.get() + 10);
    assert_eq!(read(&source), Err(QueryError::Cancelled));
    let interrupted = remaining(&pool);
    assert!(interrupted[0] < before[0] || interrupted[2] < before[2]);
    stop.set(usize::MAX);
    assert_eq!(read(&source), Ok(38));
    assert!(remaining(&pool)[0] < interrupted[0]);

    let fail = Cell::new(false);
    let source = NativeSource::new_with_shared_budget(
        Failing {
            cursor: Cursor::new(copy()),
            fail: &fail,
        },
        CatalogScope::new(9),
        &pool,
        &|| false,
    )
    .unwrap();
    source.columns("ROULETTE_TB").unwrap();
    let before = remaining(&pool);
    fail.set(true);
    assert_eq!(read(&source), Err(QueryError::Source));
    let failed = remaining(&pool);
    assert!(failed[0] < before[0] || failed[2] < before[2]);
    fail.set(false);
    assert_eq!(read(&source), Ok(38));
    assert!(remaining(&pool)[0] < failed[0]);
}

#[test]
fn setup_charges_and_explicit_row_limits_never_reset_the_pool() {
    let pool = RefCell::new(allowance(MAX_STEPS));
    let source = open(&pool, &|| false);
    let before = remaining(&pool);
    let source = source.with_stored_calculations().unwrap();
    assert!(remaining(&pool)[2] < before[2]);
    let before_index = remaining(&pool);
    let source = source.with_index("ROULETTE_TB", "ExplicitIndex").unwrap();
    assert!(remaining(&pool)[0] < before_index[0]);
    let configured = remaining(&pool);
    drop(source);
    assert_eq!(remaining(&pool), configured);

    let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
        Cursor::new(copy()),
        CatalogScope::new(8),
        &pool,
        &|| false,
        tablecraft_ace::TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap();
    let before = remaining(&pool);
    assert_eq!(read(&source), Err(QueryError::StructureLimit));
    let failed = remaining(&pool);
    assert!(failed[0] < before[0]);
    assert_eq!(read(&source), Err(QueryError::StructureLimit));
    assert!(remaining(&pool)[0] < failed[0]);
}

#[test]
fn a_host_visitor_can_read_another_source_without_a_stale_pool_borrow() {
    let pool = RefCell::new(allowance(MAX_STEPS));
    let left = open(&pool, &|| false);
    let right = open_with_scope(&pool, CatalogScope::new(8), &|| false);
    let mut visited = 0;
    assert_eq!(
        left.scan("ROULETTE_TB", &mut |_| {
            visited += 1;
            assert_eq!(read(&right), Ok(38));
            Err(QueryError::Cancelled)
        }),
        Err(QueryError::Cancelled)
    );
    assert_eq!(visited, 1);
    let completed = remaining(&pool);
    assert_eq!(read(&left), Ok(38));
    assert_eq!(read(&right), Ok(38));
    assert_eq!(remaining(&pool), completed);
}

#[test]
fn native_and_comparison_pools_cannot_alias_but_distinct_pools_remain_independent() {
    let pool = RefCell::new(allowance(MAX_STEPS));
    let cancelled = Cell::new(false);
    let cancellation = || cancelled.get();
    let source = open(&pool, &cancellation);
    let before = remaining(&pool);
    assert!(matches!(
        source.with_database_collation_resolver(&pool),
        Err(InspectError::Unsupported)
    ));
    assert_eq!(remaining(&pool), before);

    let source = open(&pool, &cancellation);
    let before = remaining(&pool);
    cancelled.set(true);
    assert!(matches!(
        source.with_database_collation_resolver(&pool),
        Err(InspectError::Cancelled)
    ));
    assert_eq!(remaining(&pool), before);

    cancelled.set(false);
    let comparisons = RefCell::new(allowance(MAX_STEPS));
    let source = open(&pool, &cancellation);
    let (source, resolver) = source
        .with_database_collation_resolver(&comparisons)
        .unwrap();
    let native_before = remaining(&pool);
    let comparison_before = remaining(&comparisons);
    assert_eq!(
        resolver.default_collation().compare("A", "a").unwrap(),
        std::cmp::Ordering::Equal
    );
    assert_eq!(remaining(&pool), native_before);
    assert!(remaining(&comparisons)[2] < comparison_before[2]);
    let comparison_after = remaining(&comparisons);
    assert_eq!(read(&source), Ok(38));
    assert_eq!(remaining(&comparisons), comparison_after);
}
