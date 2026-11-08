#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_testkit::fixtures::access::canonical_fixed_sql_before_change as model;

fn run(path: &Path, journal: &Path, sql: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "journal-update", sql])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(journal)
        .output()
        .unwrap()
}

#[test]
fn normal_fixed_sql_cli_publishes_twelve_complete_literal_images() {
    // Register/run only after the separate SOURCE20 native/SDK gates qualify.
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let statements = if case.sql == case.reversed_sql {
                vec![case.sql]
            } else {
                vec![case.sql, case.reversed_sql]
            };
            for sql in statements {
                // MONEY is reserved by the SQL lexer. Retain the frozen source
                // requests and images; quote only this target in the CLI request.
                let sql = sql.replace("Money=", "[Money]=");
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let journal = root.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let output = run(&path, &journal, &sql);
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(output.stderr.is_empty());
                assert_eq!(
                    output.stdout,
                    concat!(
                        "Transaction\tCommitted\n",
                        "Matched rows\t1\n",
                        "Changed rows\t1\n",
                        "Journal namespace\tRetired\n",
                        "Retirement guarantee\tCaptured-parent barrier\n",
                        "Journal cleanup\tRemoved\n",
                    )
                    .as_bytes()
                );
                assert_eq!(fs::read(&path).unwrap(), case.expected);
                assert!(!journal.exists());
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 12);
}

#[test]
fn journal_batch_rhs_errors_are_eager_and_never_create_a_journal() {
    let case = model::source(6, model::Recipe::Joint);
    for sql in [
        "UPDATE Items SET Flag=True,Int64Value=CLngLng(IIf(True,42,1/0)) WHERE Int32Value=0",
        "UPDATE Items SET Flag=True,Flag=False WHERE Int32Value=0",
        "UPDATE Items SET Flag=True,GuidValue=Null WHERE Int32Value=0",
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let journal = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = run(&path, &journal, sql);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!journal.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn normal_fixed_empty_batch_selection_keeps_complete_source_bytes() {
    let case = model::source(6, model::Recipe::Joint);
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target.accdb");
    let journal = root.path().join("journal");
    fs::write(&path, &case.before).unwrap();
    let output = run(
        &path,
        &journal,
        "UPDATE Items SET Flag=True,Int64Value=CLngLng(42) WHERE Int32Value=999",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        concat!(
            "Transaction\tCommitted\n",
            "Matched rows\t0\n",
            "Changed rows\t0\n",
            "Journal namespace\tRetired\n",
            "Retirement guarantee\tCaptured-parent barrier\n",
            "Journal cleanup\tRemoved\n",
        )
        .as_bytes()
    );
    assert_eq!(fs::read(&path).unwrap(), case.before);
    assert!(!journal.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[path = "normal_fixed_sql_journal/zero_matches.rs"]
mod zero_matches;
