#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};
use tablecraft_ace::{FileFormat, Int32Update, NativeImage, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal::filesystem::{OfflineJournalPair, OfflineTarget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::mutation_fixture;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn images() -> (Vec<u8>, Vec<u8>) {
    let before = mutation_fixture::database();
    let mut after = before.clone();
    // Independent first-row Score location in the public Jet4 fixture.
    let score = 12 * 4096 + 4091;
    assert_eq!(&before[score..score + 4], &10_i32.to_le_bytes());
    after[score..score + 4].copy_from_slice(&12_i32.to_le_bytes());
    assert_eq!(&before[..score], &after[..score]);
    assert_eq!(&before[score + 4..], &after[score + 4..]);

    (before, after)
}

fn apply(database: &Path, journal: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "journal-update",
            "UPDATE Items SET Score=12 WHERE Id=1",
        ])
        .arg("--database")
        .arg(database)
        .arg("--journal")
        .arg(journal)
        .output()
        .unwrap()
}

#[test]
fn qualify_journal_cli_originals_and_complete_expected_before_commands() {
    let (before, after) = images();
    for (bytes, score) in [(&before, 10), (&after, 12)] {
        let image = NativeImage::capture(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(image.catalog().header().format(), FileFormat::Jet4);
        let checked = check_table(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 3);
        assert!(checked.indexes().is_empty());
        let rows = read_table(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows(),
            &[
                vec![Value::Int32(1), Value::Int32(score)],
                vec![Value::Int32(2), Value::Int32(20)],
                vec![Value::Int32(3), Value::Int32(30)],
            ]
        );
    }
}

#[test]
fn journal_update_commits_complete_image_and_retires_active_bundle() {
    let (before, after) = images();
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("source.mdb");
    let journal = root.path().join("active-journal");
    fs::write(&database, &before).unwrap();

    let output = apply(&database, &journal);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
    assert_eq!(fs::read(&database).unwrap(), after);
    assert!(!journal.exists());
}

#[test]
fn existing_journal_and_missing_input_preserve_every_existing_byte() {
    let (before, _) = images();
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("source.mdb");
    let journal = root.path().join("active-journal");
    fs::write(&database, &before).unwrap();
    fs::create_dir(&journal).unwrap();
    fs::write(journal.join("sentinel"), b"existing namespace").unwrap();

    let output = apply(&database, &journal);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&database).unwrap(), before);
    assert_eq!(
        fs::read(journal.join("sentinel")).unwrap(),
        b"existing namespace"
    );
    assert_eq!(fs::read_dir(&journal).unwrap().count(), 1);

    let missing = root.path().join("missing.mdb");
    let fresh_journal = root.path().join("fresh-journal");
    let output = apply(&missing, &fresh_journal);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!missing.exists());
    assert!(!fresh_journal.exists());
    assert_eq!(fs::read(&database).unwrap(), before);
}

#[test]
fn unsupported_sql_and_source_refuse_without_creating_a_bundle() {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("source.mdb");
    let journal = root.path().join("active-journal");
    let original = mutation_fixture::database();
    fs::write(&database, &original).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "journal-update",
            "UPDATE Items SET Score=12, Id=4 WHERE Id=1",
        ])
        .arg("--database")
        .arg(&database)
        .arg("--journal")
        .arg(&journal)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&database).unwrap(), original);
    assert!(!journal.exists());

    let unsupported =
        mutation_fixture::with_rule("UnqualifiedJournalProperty", "opaque source data");
    fs::write(&database, &unsupported).unwrap();
    let output = apply(&database, &journal);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&database).unwrap(), unsupported);
    assert!(!journal.exists());
}

fn leave_actual_pair_for_recovery(database: &Path, journal_path: &Path, committed: bool) {
    let (before, after) = images();
    fs::write(database, &before).unwrap();
    let mut target = OfflineTarget::open(database, &mut budget(), &|| false).unwrap();
    let image =
        NativeImage::capture(&mut target, CatalogScope::new(1), &mut budget(), &|| false).unwrap();
    // Only the real unchanged normal native planner constructs authority. Pair
    // creation stores its exact encoded intent; no journal/binding is forged.
    let plan = image
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, 12)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), after);
    let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
    let mut pair =
        OfflineJournalPair::create(target, journal_path, intent, &mut budget(), &|| false).unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
        } else {
            // Simulate an interrupted prepared operation. Drop performs no
            // rollback/retirement; the actual pending record remains recoverable.
            drop(prepared);
        }
    }
    pair.release().unwrap();
    drop(pair);
    assert!(journal_path.is_dir());
    assert_eq!(
        fs::read(database).unwrap(),
        if committed { after } else { before }
    );
}

#[test]
fn recover_update_uses_real_pending_and_committed_pairs_then_retires_them() {
    for committed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("source.mdb");
        let journal = root.path().join("active-journal");
        leave_actual_pair_for_recovery(&database, &journal, committed);

        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "recover-update"])
            .arg("--database")
            .arg(&database)
            .arg("--journal")
            .arg(&journal)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let expected_report: &[u8] = if committed {
            b"Recovery\tCommitted\nJournal namespace\tRetired and synchronized\n"
        } else {
            b"Recovery\tRolledBack\nJournal namespace\tRetired and synchronized\n"
        };
        assert_eq!(output.stdout, expected_report);
        let (before, after) = images();
        assert_eq!(
            fs::read(&database).unwrap(),
            if committed { after } else { before }
        );
        assert!(!journal.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[path = "journal_editing/finalization.rs"]
mod finalization;
