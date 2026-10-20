use super::*;
use crate::SqliteError;
use std::{
    cmp::Ordering as Comparison,
    future::{Future, poll_fn},
    path::PathBuf,
    sync::{Condvar, Mutex, atomic::AtomicU64},
    task::Poll,
    time::Duration,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::QueryError;
use tokio::sync::Notify;

const TEST_TIMEOUT: Duration = Duration::from_secs(10);
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-sqlite-worker-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!path.exists());
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE Items(ID INTEGER, Label TEXT); INSERT INTO Items VALUES(1,'b'),(2,'a')").unwrap();
        connection.close().unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn options() -> AsyncOptions {
    AsyncOptions::new(
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        Arc::new(AtomicBool::new(false)),
    )
}

fn query(sql: &str) -> Query {
    tablecraft_sql::parse_query(sql, Default::default())
        .unwrap()
        .query()
        .clone()
}

#[tokio::test(flavor = "current_thread")]
async fn worker_executes_access_queries_and_owns_one_snapshot_per_request() {
    let fixture = Fixture::new();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let result = reader.query(query("PARAMETERS Floor BIGINT; SELECT TOP 1 ID, Label & '!' AS Caption FROM Items WHERE ID > Floor ORDER BY ID DESC"),
        vec![("Floor".into(), Value::Int64(0))], options()).await.unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Int64(2), Value::Text("a!".into())]]
    );

    // The completed job must have released its read transaction before accepting
    // another job; a separate rollback-journal writer can commit immediately.
    let writer = rusqlite::Connection::open(&fixture.0).unwrap();
    writer.busy_timeout(Duration::ZERO).unwrap();
    writer
        .execute("INSERT INTO Items VALUES(3,'c')", [])
        .unwrap();
    writer.close().unwrap();
    let result = reader
        .query(query("SELECT Count(*) AS N FROM Items"), vec![], options())
        .await
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Int64(3)]]);
    reader.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn failed_cancelled_and_exhausted_requests_clean_up_for_reuse() {
    let fixture = Fixture::new();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let error = reader
        .query(query("SELECT * FROM Missing"), vec![], options())
        .await;
    assert!(matches!(
        error,
        Err(AsyncError::Query(QueryError::UnknownName))
    ));

    let cancelled = options();
    cancelled.cancelled.store(true, Ordering::Relaxed);
    assert!(matches!(
        reader
            .query(query("SELECT * FROM Items"), vec![], cancelled)
            .await,
        Err(AsyncError::Storage(SqliteError::Cancelled))
    ));
    let mut limited = options();
    limited.storage_budget = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        reader
            .query(query("SELECT * FROM Items"), vec![], limited)
            .await,
        Err(AsyncError::Storage(SqliteError::Limit(_)))
    ));
    let result = reader
        .query(query("SELECT Count(*) AS N FROM Items"), vec![], options())
        .await
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Int64(2)]]);
    reader.close().await.unwrap();
}

struct BlockingCollation {
    entered: Notify,
    released: Mutex<bool>,
    wake: Condvar,
}
impl BlockingCollation {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
impl Collation for BlockingCollation {
    fn compare(&self, left: &str, right: &str) -> Result<Comparison, EvalError> {
        self.entered.notify_one();
        let released = self.released.lock().unwrap();
        let (_released, timeout) = self
            .wake
            .wait_timeout_while(released, TEST_TIMEOUT, |released| !*released)
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "test failed to release worker collation"
        );
        Ok(left.cmp(right))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_a_future_cancels_but_keeps_admission_until_worker_cleanup() {
    let fixture = Fixture::new();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let collation = Arc::new(BlockingCollation {
        entered: Notify::new(),
        released: Mutex::new(false),
        wake: Condvar::new(),
    });
    let mut settings = options();
    settings.collation = Some(collation.clone());
    let cancelled = settings.cancelled.clone();
    let mut request = Box::pin(reader.query(
        query("SELECT Label FROM Items ORDER BY Label"),
        vec![],
        settings,
    ));
    poll_fn(|context| {
        assert!(request.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    tokio::time::timeout(TEST_TIMEOUT, collation.entered.notified())
        .await
        .unwrap();

    assert!(matches!(
        reader.query(query("SELECT 1"), vec![], options()).await,
        Err(AsyncError::Busy)
    ));
    drop(request);
    assert!(cancelled.load(Ordering::Relaxed));
    // The worker is deliberately still inside the host callback. Dropping the
    // future must not release admission or silently queue this second request.
    assert!(matches!(
        reader.query(query("SELECT 1"), vec![], options()).await,
        Err(AsyncError::Busy)
    ));
    collation.release();
    let permit = tokio::time::timeout(TEST_TIMEOUT, reader.gate.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    let result = reader
        .query(query("SELECT Count(*) AS N FROM Items"), vec![], options())
        .await
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Int64(2)]]);
    reader.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn nonexistent_paths_are_not_created_by_async_open() {
    let fixture = Fixture::new();
    let missing = fixture.0.with_extension("missing");
    assert!(matches!(
        AsyncReader::open(&missing).await,
        Err(AsyncError::Storage(SqliteError::Database))
    ));
    assert!(!missing.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn worker_forwards_like_mode_without_using_sqlite_pattern_semantics() {
    struct Ordinal;
    impl Collation for Ordinal {
        fn compare(&self, a: &str, b: &str) -> Result<Comparison, EvalError> {
            Ok(a.cmp(b))
        }
    }
    let fixture = Fixture::new();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    for (mode, pattern) in [
        (tablecraft_expressions::predicates::LikeMode::Ansi89, "a*"),
        (tablecraft_expressions::predicates::LikeMode::Ansi92, "a%"),
    ] {
        let mut settings = options();
        settings.like_mode = Some(mode);
        settings.collation = Some(Arc::new(Ordinal));
        let result = reader
            .query(
                query(&format!(
                    "SELECT ID FROM Items WHERE Label LIKE '{pattern}'"
                )),
                vec![],
                settings,
            )
            .await
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int64(2)]]);
    }
    reader.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn year_windows_stay_with_each_worker_request() {
    use tablecraft_expressions::functions::dates::TwoDigitYearWindow;
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT Year(DateSerial(49,1,1)) AS Y");
    for (window, expected) in [
        (None, 1949),
        (Some(TwoDigitYearWindow::new(2049).unwrap()), 2049),
        (None, 1949),
    ] {
        let mut options = options();
        options.year_window = window;
        let result = reader.query(sql.clone(), vec![], options).await.unwrap();
        assert_eq!(result.rows(), [vec![Value::Int16(expected)]]);
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn character_code_pages_stay_with_each_worker_request() {
    use tablecraft_expressions::functions::text::code_page::CharacterCodePage;
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT String(2,192)");
    for (page, expected) in [
        (Some(1252), Some("ÀÀ")),
        (Some(1251), Some("АА")),
        (None, None),
        (Some(1252), Some("ÀÀ")),
    ] {
        let mut settings = options();
        settings.character_code_page =
            page.map(|id| CharacterCodePage::from_windows_id(id).unwrap());
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Text(expected.into())]]);
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}
