#![cfg(feature = "journal")]

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_access::editing::sql;
use tablecraft_ace::{NativeImage, NativeInt32EditIntent, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::{
    IoError,
    journal::{DurableStorage, RecoveryOutcome, inspect_v1},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::mutation_fixture as fixture;

// Independent fixture location; no production layout constant constructs this.
const SECOND_ROW_SCORE_OFFSET: usize = 12 * 4096 + 4080;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

struct Storage(Cursor<Vec<u8>>);

impl Read for Storage {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes)
    }
}

impl Write for Storage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush is not a durability barrier")
    }
}

impl Seek for Storage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

impl DurableStorage for Storage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.0.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        Ok(())
    }
}

#[test]
fn sql_recoverable_plan_retains_resolved_requests_and_commits_exact_image() {
    let before = fixture::with_rule("ValidationRule", ">=0 And <=100");
    let mut expected = before.clone();
    let offset = SECOND_ROW_SCORE_OFFSET;
    assert_eq!(&expected[offset..offset + 4], &20_i32.to_le_bytes());
    expected[offset..offset + 4].copy_from_slice(&27_i32.to_le_bytes());
    let checked = read_table(
        &mut Cursor::new(&expected),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows()[1], [Value::Int32(2), Value::Int32(27)]);

    let query = tablecraft_sql::parse_query(
        "UPDATE Items SET Score=Score+7 WHERE Id=2",
        tablecraft_sql::ParseLimits::default(),
    )
    .unwrap();
    let plain = sql::plan_update(image(&before), query.query(), &mut budget(), None, &|| {
        false
    })
    .unwrap();
    let recorded =
        sql::plan_recoverable_update(image(&before), query.query(), &mut budget(), None, &|| {
            false
        })
        .unwrap();
    assert_eq!(plain.matched_rows(), recorded.matched_rows());
    let recorded = recorded.into_backend();
    assert_eq!(recorded.before_bytes(), before);
    assert_eq!(recorded.after_bytes(), expected);
    assert_eq!(plain.into_backend().after_bytes(), recorded.after_bytes());
    assert_eq!(recorded.changed_rows(), 1);
    let intent = recorded.intent();
    assert_eq!((intent.table(), intent.column()), ("Items", "Score"));
    assert_eq!(intent.updates().len(), 1);
    let update = &intent.updates()[0];
    assert_eq!(
        (update.row(), update.expected(), update.value()),
        (1, 20, 27)
    );
    let encoded = intent.encode(&mut budget(), &|| false).unwrap();
    let decoded = NativeInt32EditIntent::decode(&encoded, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();

    let mut target = Storage(Cursor::new(before));
    let mut journal = Storage(Cursor::new(Vec::new()));
    let prepared = recorded
        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
    assert_eq!(target.0.get_ref(), &expected);
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert_eq!(checked.after_bytes(), expected);
    assert_eq!(
        checked
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::Committed
    );
    assert_eq!(target.0.get_ref(), &expected);
}

#[test]
fn sql_recoverable_planning_keeps_noop_intent_and_native_refusals() {
    let before = fixture::with_rule("ValidationRule", ">=0 And <=100");
    for (statement, count) in [
        ("UPDATE Items SET Score=Score WHERE Id=2", 1),
        ("UPDATE Items SET Score=Score+7 WHERE Id=99", 0),
    ] {
        let query =
            tablecraft_sql::parse_query(statement, tablecraft_sql::ParseLimits::default()).unwrap();
        let plan = sql::plan_recoverable_update(
            image(&before),
            query.query(),
            &mut budget(),
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), count);
        let native = plan.into_backend();
        assert_eq!(native.changed_rows(), 0);
        assert_eq!(native.before_bytes(), native.after_bytes());
        assert_eq!(native.intent().updates().len(), count);
    }

    for statement in [
        "UPDATE Items SET Score=101 WHERE Id=2",
        "UPDATE Items SET Score=NULL WHERE Id=2",
        "UPDATE Items SET Score=27 WHERE Id=2 OR 1/0=0",
    ] {
        let query =
            tablecraft_sql::parse_query(statement, tablecraft_sql::ParseLimits::default()).unwrap();
        assert!(
            sql::plan_recoverable_update(
                image(&before),
                query.query(),
                &mut budget(),
                None,
                &|| false,
            )
            .is_err()
        );
    }
}
