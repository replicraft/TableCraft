//! Actual Access/CLI consumers of the already qualified rich source owner.
use super::{SCOPE, budget, catalog_flag, model};
use std::{fs, io::Cursor, process::Command};
use tablecraft_access::engineering::{ScalarUpdateMode, sql};
use tablecraft_ace::NativeImage;
use tablecraft_sql::{ParseLimits, parse_query};

const SQL: &str = "UPDATE Items SET Yes_Field=NOT Yes_Field WHERE ID=1";

#[test]
fn preserved_boolean_access_and_actual_sql_typed_copies_match_one_bit_expectations() {
    for case in [model::source(), catalog_flag::source()] {
        let query = parse_query(SQL, ParseLimits::default()).unwrap();
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = sql::preview_batch_update(
            image,
            query.query(),
            ScalarUpdateMode::Default,
            &mut budget(),
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.into_backend().candidate_bytes(), case.expected);

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        fs::write(&source, &case.before).unwrap();
        for typed in [false, true] {
            let output = directory
                .path()
                .join(if typed { "typed.accdb" } else { "sql.accdb" });
            let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
            if typed {
                command.args([
                    "write",
                    "preview-update",
                    "--set",
                    "Items",
                    "Yes_Field",
                    "0",
                    "boolean",
                    "false",
                    "true",
                ]);
            } else {
                command.args(["query", "preview-update", SQL]);
            }
            if typed {
                command.arg(&source).arg(&output);
            } else {
                command
                    .arg("--database")
                    .arg(&source)
                    .arg("--output")
                    .arg(&output);
            }
            let result = command.output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(fs::read(&output).unwrap(), case.expected);
            assert_eq!(fs::read(&source).unwrap(), case.before);
        }
    }
}

#[test]
fn preserved_cli_refusals_stale_expectations_and_fresh_name_boundaries_leave_no_artifacts() {
    let case = catalog_flag::source();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    fs::write(&source, &case.before).unwrap();
    for (ordinal, statement) in [
        "UPDATE Items SET Number=12 WHERE ID=1",
        "UPDATE Items SET Number=12 WHERE ID=-1",
        "UPDATE Items SET Calculated=12 WHERE ID=1",
        "UPDATE Items SET Yes_Field=Calculated>0 WHERE ID=1",
        "UPDATE Items SET Yes_Field=True WHERE Attachment_Field IS NULL",
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused-{ordinal}.accdb"));
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "preview-update", statement, "--database"])
            .arg(&source)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
        assert_eq!(fs::read(&source).unwrap(), case.before);
    }

    let stale = directory.path().join("stale.accdb");
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["write", "preview-update"])
        .arg(&source)
        .arg(&stale)
        .args([
            "--set",
            "Items",
            "Yes_Field",
            "0",
            "boolean",
            "true",
            "false",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!stale.exists());

    let wrong_mode = directory.path().join("wrong-mode.accdb");
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", SQL, "--variable", "--database"])
        .arg(&source)
        .arg("--output")
        .arg(&wrong_mode)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!wrong_mode.exists());

    let existing = directory.path().join("existing.accdb");
    fs::write(&existing, b"existing destination").unwrap();
    for output in [&source, &existing] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "preview-update", SQL, "--database"])
            .arg(&source)
            .arg("--output")
            .arg(output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert_eq!(fs::read(&source).unwrap(), case.before);
        assert_eq!(fs::read(&existing).unwrap(), b"existing destination");
    }
}
