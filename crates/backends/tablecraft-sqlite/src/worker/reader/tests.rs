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

#[tokio::test(flavor = "current_thread")]
async fn number_format_stays_with_each_worker_request() {
    use tablecraft_expressions::functions::conversions::text::NumberFormat;
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT CLng('1,234')");
    for (format, expected) in [
        (Some(NumberFormat::EnUs), Some(1234)),
        (Some(NumberFormat::DeDe), Some(1)),
        (None, None),
        (Some(NumberFormat::EnUs), Some(1234)),
    ] {
        let mut settings = options();
        settings.number_format = format;
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Int32(expected)]]);
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn numeric_inspection_retains_request_policy_and_reuses_after_refusal() {
    use tablecraft_expressions::functions::conversions::text::NumberFormat;
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT IsNumeric('$1')");
    for (format, expected) in [
        (Some(NumberFormat::EnUs), Some(true)),
        (Some(NumberFormat::DeDe), Some(false)),
        (None, None),
        (Some(NumberFormat::EnUs), Some(true)),
    ] {
        let mut settings = options();
        settings.number_format = format;
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Boolean(expected)]]);
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn cstr_date_and_numeric_policies_are_request_local_and_failures_allow_reuse() {
    use tablecraft_expressions::functions::{
        conversions::text::NumberFormat, text::cstr::DateTextFormat,
    };
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT CStr(#2026-10-20#),CStr(1.25)");
    for (date, number, expected) in [
        (
            Some(DateTextFormat::Iso),
            Some(NumberFormat::DeDe),
            Some(("2026-10-20", "1,25")),
        ),
        (
            Some(DateTextFormat::EnUs),
            Some(NumberFormat::Invariant),
            Some(("10/20/2026", "1.25")),
        ),
        (None, Some(NumberFormat::Invariant), None),
        (
            Some(DateTextFormat::DeDe),
            Some(NumberFormat::EnUs),
            Some(("20.10.2026", "1.25")),
        ),
    ] {
        let mut settings = options();
        settings.date_format = date;
        settings.number_format = number;
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some((date, number)) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Text(date.into()), Value::Text(number.into())]]
            );
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn date_conversion_reference_year_does_not_leak_across_requests() {
    use tablecraft_expressions::functions::dates::parsing::{DateTextFormat, ReferenceYear};
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT Year(DateValue('Feb 29'))");
    for (year, expected) in [
        (Some(2024), Some(2024)),
        (Some(2028), Some(2028)),
        (None, None),
        (Some(2024), Some(2024)),
    ] {
        let mut settings = options();
        settings.date_format = Some(DateTextFormat::EnUs);
        settings.reference_year = year.map(|year| ReferenceYear::new(year).unwrap());
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Int16(expected)]]);
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn date_inspection_reuses_worker_after_missing_policy_and_invalid_dates() {
    use tablecraft_expressions::functions::dates::parsing::{DateTextFormat, ReferenceYear};
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT IsDate('Feb 29')");
    for (year, expected) in [
        (Some(2024), Some(true)),
        (Some(2023), Some(false)),
        (None, None),
        (Some(2024), Some(true)),
    ] {
        let mut settings = options();
        settings.date_format = Some(DateTextFormat::EnUs);
        settings.reference_year = year.map(|year| ReferenceYear::new(year).unwrap());
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Boolean(expected)]]);
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn date_formatting_keeps_worker_input_and_output_settings_request_local() {
    use tablecraft_expressions::functions::dates::parsing::{DateTextFormat, ReferenceYear};
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT FormatDateTime('Feb 29',2)");
    for (profile, year, expected) in [
        (DateTextFormat::EnUs, Some(2024), Some("2/29/2024")),
        (DateTextFormat::DeDe, Some(2024), Some("29.02.2024")),
        (DateTextFormat::Iso, None, None),
        (DateTextFormat::Iso, Some(2028), Some("2028-02-29")),
    ] {
        let mut settings = options();
        settings.date_format = Some(profile);
        settings.reference_year = year.map(|year| ReferenceYear::new(year).unwrap());
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

#[tokio::test(flavor = "current_thread")]
async fn calendar_names_keep_language_and_week_settings_request_local() {
    use tablecraft_expressions::functions::dates::{FirstDayOfWeek, parsing::DateTextFormat};
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT MonthName(3,True),WeekdayName(1)");
    for (profile, first, expected) in [
        (DateTextFormat::EnUs, Some(1), Some(["Mar", "Sunday"])),
        (DateTextFormat::DeDe, Some(2), Some(["Mär", "Montag"])),
        (DateTextFormat::DeDe, None, None),
        (DateTextFormat::Iso, Some(7), Some(["Mar", "Saturday"])),
    ] {
        let mut settings = options();
        settings.date_format = Some(profile);
        settings.first_day_of_week = first.map(|day| FirstDayOfWeek::new(day).unwrap());
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some(expected) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [expected.map(|text| Value::Text(text.into())).to_vec()]
            );
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn system_week_requests_keep_host_settings_isolated_and_recover_after_failure() {
    use tablecraft_expressions::functions::dates::{FirstDayOfWeek, FirstWeekOfYear};
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query(
        "SELECT DatePart('ww',#2021-01-01#,0,0),DateDiff('ww',#2026-10-18#,#2026-10-19#,0,0)",
    );
    for (day, rule, expected) in [
        (1, Some(1), Some((1, 0))),
        (2, Some(2), Some((53, 1))),
        (2, None, None),
        (2, Some(3), Some((52, 1))),
    ] {
        let mut settings = options();
        settings.first_day_of_week = Some(FirstDayOfWeek::new(day).unwrap());
        settings.first_week_of_year = rule.map(|rule| FirstWeekOfYear::new(rule).unwrap());
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some((part, difference)) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Int16(part), Value::Int32(difference)]]
            );
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn weekday_context_and_text_year_remain_request_local() {
    use tablecraft_expressions::functions::dates::{
        FirstDayOfWeek,
        parsing::{DateTextFormat, ReferenceYear},
    };
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT Weekday('Feb 29',0)");
    for (day, year, expected) in [
        (2, Some(2024), Some(4)),
        (1, Some(2024), Some(5)),
        (2, None, None),
        (2, Some(2028), Some(2)),
    ] {
        let mut settings = options();
        settings.date_format = Some(DateTextFormat::EnUs);
        settings.first_day_of_week = Some(FirstDayOfWeek::new(day).unwrap());
        settings.reference_year = year.map(|year| ReferenceYear::new(year).unwrap());
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Int16(expected)]]);
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn styled_numeric_output_is_request_local_and_recovers_after_profile_failure() {
    use tablecraft_expressions::functions::conversions::text::NumberFormat;
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT FormatCurrency(-1234.5)");
    for (profile, expected) in [
        (NumberFormat::EnUs, Some("($1,234.50)")),
        (NumberFormat::DeDe, Some("-1.234,50 €")),
        (NumberFormat::Invariant, None),
        (NumberFormat::EnUs, Some("($1,234.50)")),
    ] {
        let mut settings = options();
        settings.number_format = Some(profile);
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

#[tokio::test(flavor = "current_thread")]
async fn clock_providers_are_request_local_and_recover_after_missing_context() {
    use tablecraft_core::values::ExtendedDateTime;
    use tablecraft_expressions::functions::dates::clock::ClockReading;
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT Year(Now()),Hour(Time())");
    for input in [Some((2024, 1)), Some((2026, 23)), None, Some((2028, 12))] {
        let mut settings = options();
        settings.clock = input.map(|(year, hour)| {
            Arc::new(
                ClockReading::new(ExtendedDateTime::new(year, 10, 20, hour, 0, 0, 0).unwrap())
                    .unwrap(),
            ) as Arc<dyn tablecraft_expressions::functions::dates::clock::Clock>
        });
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some((year, hour)) = input {
            assert_eq!(
                result.unwrap().rows(),
                [vec![
                    Value::Int16(year as i16),
                    Value::Int16(i16::from(hour))
                ]]
            );
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}

#[tokio::test(flavor = "current_thread")]
async fn casing_is_request_local_and_recovers_after_missing_policy() {
    let fixture = Fixture::new();
    let original = std::fs::read(&fixture.0).unwrap();
    let reader = AsyncReader::open(&fixture.0).await.unwrap();
    let sql = query("SELECT LCase('I'),UCase('i')");
    for (locale, expected) in [
        (Some("tr"), Some(("ı", "İ"))),
        (None, None),
        (Some("und"), Some(("i", "I"))),
    ] {
        let mut settings = options();
        settings.case_locale = locale.map(|text| text.parse().unwrap());
        let result = reader.query(sql.clone(), vec![], settings).await;
        if let Some((lower, upper)) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Text(lower.into()), Value::Text(upper.into())]]
            );
        } else {
            assert!(result.is_err());
        }
    }
    reader.close().await.unwrap();
    assert_eq!(std::fs::read(&fixture.0).unwrap(), original);
}
