#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_core::values::Value;
use tablecraft_testkit::{
    fixtures::access::genuine_generated_companion_update::{self as source, Recipe},
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires twelve independently qualified generated-companion Score images"]
fn cli_explicit_score_updates_commit_exact_counter_default_deleted_and_index_bytes() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut saves = 0;
    for case in &cases {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let journal = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = CLI.run(&[
            "query",
            "journal-update",
            case.sql(),
            "--database",
            path.to_str().unwrap(),
            "--journal",
            journal.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let expected = format!(
            "Transaction\tCommitted\n\
                Matched rows\t{}\n\
                Changed rows\t{}\n\
                Journal namespace\tRetired\n\
                Retirement guarantee\tCaptured-parent barrier\n\
                Journal cleanup\tRemoved\n",
            case.matched(),
            case.changed()
        );
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());
        case.assert_preserved();
        let (sql, mut expected) = if case.numbered() {
            (
                "SELECT [Earlier],[Id],[Score] FROM [Numbered]",
                String::from("Earlier\tId\tScore\n"),
            )
        } else {
            (
                "SELECT [Id],[Score] FROM [Items]",
                String::from("Id\tScore\n"),
            )
        };
        for row in case.rows() {
            let values: Vec<_> = row
                .into_iter()
                .map(|value| match value {
                    Value::Int32(value) => value.to_string(),
                    Value::Null => "NULL".to_owned(),
                    _ => panic!("only source-qualified I32/NULL result carriers"),
                })
                .collect();
            expected.push_str(&values.join("\t"));
            expected.push('\n');
        }
        let read = CLI.run(&["query", "run", sql, "--database", path.to_str().unwrap()]);
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        assert_eq!(read.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 16);
}

#[test]
#[ignore = "requires four independently qualified original generated-companion schemas"]
fn cli_generated_value_noop_null_empty_and_real_score_overflow_leave_no_output_or_namespace() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut refusals = 0;
    for case in cases.iter().filter(|case| case.recipe == Recipe::Changed) {
        for (column, value, id) in [
            ("Id", "4", 1),
            ("Id", "1", 1),
            ("Id", "NULL", 1),
            ("Id", "4", 999),
            ("Score", "2147483648", 1),
        ] {
            // SQL accepts checked integral assignment to Int32. The final
            // request is actual overflow, not a false tiny-carrier rejection.
            let sql = format!(
                "UPDATE [{}] SET [{column}]={value} WHERE [Id]={id}",
                case.table
            );
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let journal = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let output = CLI.run(&[
                "query",
                "journal-update",
                &sql,
                "--database",
                path.to_str().unwrap(),
                "--journal",
                journal.to_str().unwrap(),
            ]);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!journal.exists());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            refusals += 1;
        }
    }
    assert_eq!(refusals, 20);
}
