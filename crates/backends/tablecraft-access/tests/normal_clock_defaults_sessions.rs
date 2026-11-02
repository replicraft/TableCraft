#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
//! Actual typed and SQL retained sessions use the same explicit provider boundary.
use sha2::{Digest, Sha256};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_access::editing::offline::{OfflineWriteSession, Phase};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertValue};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::normal_clock_defaults::{self as fixtures, Field};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

struct Provider {
    reading: ClockReading,
    allowed: bool,
    calls: AtomicUsize,
}
impl Provider {
    fn new(case: &fixtures::Case<'_>) -> Self {
        Self {
            reading: ClockReading::new(case.clock_input()).unwrap(),
            allowed: case.needs_clock(),
            calls: AtomicUsize::new(0),
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}
impl Clock for Provider {
    fn read(&self) -> Result<ClockReading, EvalError> {
        if self.calls.fetch_add(1, Ordering::Relaxed) == 0 && self.allowed {
            Ok(self.reading)
        } else {
            Err(EvalError::ClockUnavailable)
        }
    }
}

#[test]
#[ignore = "requires independent clock source18 native and SDK qualification"]
fn typed_and_sql_clock_saves_keep_exact_images_one_read_receipts_and_lease_after_refusal() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    let mut saves = 0;
    for case in sources
        .cases()
        .into_iter()
        .filter(|case| case.sql().is_some())
    {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(254), &mut budget(), &|| false)
                    .unwrap();
            let provider = Provider::new(&case);
            let staged = if typed {
                let rows = case.requests();
                assert_eq!(rows.len(), 1);
                let values = rows
                    .into_iter()
                    .next()
                    .unwrap()
                    .into_iter()
                    .map(|field| match field {
                        Field::Omitted => InsertValue::Omitted,
                        Field::Supplied(value) => InsertValue::Supplied(value),
                    })
                    .collect();
                session
                    .insert_table("Items", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert_with_omissions_and_clock(values, &provider, &mut budget(), &|| {
                        false
                    })
                    .unwrap()
            } else {
                let parsed = parse_query(case.sql().unwrap(), ParseLimits::default()).unwrap();
                session
                    .plan_insert_with_clock(parsed.query(), &provider, &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(staged.inserted_rows(), 1);
            assert_eq!(provider.calls(), usize::from(case.needs_clock()));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(provider.calls(), usize::from(case.needs_clock()));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(EditableTable::snapshot(&editor).rows(), case.rows());
            let session = editor.into_session();
            let untouched_clock = Provider::new(&case);
            let failure = session
                .insert_table("Items", &mut budget(), &|| false)
                .unwrap()
                .plan_insert_with_omissions_and_clock(
                    vec![InsertValue::Supplied(Value::Int32(99))],
                    &untouched_clock,
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), Some(receipt));
            assert_eq!(untouched_clock.calls(), 0);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 24);
}
