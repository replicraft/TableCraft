#![cfg(any(target_os = "macos", target_os = "linux"))]
//! Existing journal-insert automatically owns a clock; --clock only fixes input.
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::access::normal_clock_defaults::{self as fixtures, Function, Recipe};

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

fn expected(case: &fixtures::Case<'_>) -> String {
    let mut text = String::from("Id\tWhen\n1\t#1899-12-31 06:00:00.000000000#\n");
    let value = match (case.function, case.recipe) {
        (_, Recipe::SuppliedNull) => "NULL",
        (Function::Date, Recipe::BeforeEpochSix) => "#1899-12-29 00:00:00.000000000#",
        (Function::Now, Recipe::BeforeEpochSix) => "#1899-12-29 06:00:00.000000000#",
        (Function::Date, Recipe::LeapNoon) => "#2000-02-29 00:00:00.000000000#",
        (Function::Now, Recipe::LeapNoon) => "#2000-02-29 12:00:00.000000000#",
        _ => panic!("single-row CLI source context"),
    };
    let identity = if case.recipe == Recipe::SuppliedNull {
        3
    } else {
        2
    };
    text.push_str(&format!("{identity}\t{value}\n"));
    text
}

#[test]
#[ignore = "requires clock source18 native/SDK qualification before existing CLI saves"]
fn journal_insert_saves_twelve_exact_images_and_reads_back_literal_civil_values() {
    let sources = sources();
    let mut saves = 0;
    for case in sources
        .cases()
        .into_iter()
        .filter(|case| case.sql().is_some())
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, case.before).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        command
            .args(["query", "journal-insert", case.sql().unwrap()])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle);
        if case.needs_clock() {
            command.args(["--clock", case.clock_text()]);
        }
        let saved = command.output().unwrap();
        assert!(
            saved.status.success(),
            "{}",
            String::from_utf8_lossy(&saved.stderr)
        );
        assert!(saved.stderr.is_empty());
        assert_eq!(saved.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT [Id], [When] FROM Items"])
            .arg("--database")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        assert_eq!(read.stdout, expected(&case).as_bytes());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 12);
}

#[test]
#[ignore = "requires qualified clock sources for unchanged-target argument/capacity refusals"]
fn invalid_clock_override_and_healthy_full_directory_do_not_publish_a_bundle() {
    let sources = sources();
    let case = sources
        .cases()
        .into_iter()
        .find(|case| {
            case.header == 5 && case.function == Function::Now && case.recipe == Recipe::LeapNoon
        })
        .unwrap();
    for full in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        let mut original = case.before.to_vec();
        if full {
            original[8 * 4096 + 12..8 * 4096 + 14].copy_from_slice(&255_u16.to_le_bytes());
            original[8 * 4096 + 2..8 * 4096 + 4].copy_from_slice(&3557_u16.to_le_bytes());
            for slot in 1..255 {
                let at = 8 * 4096 + 14 + slot * 2;
                original[at..at + 2].copy_from_slice(&0x8ff1_u16.to_le_bytes());
            }
        }
        fs::write(&path, &original).unwrap();
        let rejected = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-insert", case.sql().unwrap()])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle)
            .args([
                "--clock",
                if full {
                    case.clock_text()
                } else {
                    "not-a-civil-reading"
                },
            ])
            .output()
            .unwrap();
        assert_eq!(rejected.status.code(), Some(2));
        assert!(rejected.stdout.is_empty());
        let diagnostic = String::from_utf8(rejected.stderr).unwrap();
        assert!(diagnostic.contains(if full {
            "RowLayout"
        } else {
            "expected local YYYY-MM-DDTHH:MM:SS"
        }));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
