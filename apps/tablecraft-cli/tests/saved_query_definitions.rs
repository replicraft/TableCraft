#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "support/mod.rs"]
mod cli_support;
#[path = "../../../crates/engines/tablecraft-ace/tests/saved_queries/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

#[path = "saved_query_definitions/crosstabs.rs"]
mod crosstabs;
#[path = "saved_query_definitions/execution.rs"]
mod execution;
#[path = "saved_query_definitions/unions.rs"]
mod unions;

use fixture::Fragment;
use std::{
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Database {
    path: PathBuf,
    bytes: Vec<u8>,
}
impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-saved-query-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        Self { path, bytes }
    }
    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        std::fs::remove_file(&self.path).unwrap();
    }
}

#[test]
fn explicit_saved_query_inspection_retains_inert_fragments_and_quoted_null_distinctions() {
    let db = Database::new(fixture::database());
    let result = cli_support::run(&["inspect", db.path(), "--query", "Saved"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.starts_with(&format!(
        "Query\tSaved\nNative ID\t{}\nFlags\t0x80000030\nFragment rows\t2\n",
        fixture::QUERY_ID
    )));
    assert!(text.contains(
        "200\t\"\"\"NeverRunMe()\\t\\n\\u{0000}é🦀\"\"\"\t-32768\t-2147483648\t\"\"\"NULL\"\"\"\t\"\"\"\"\"\"\t0x09080706\n"
    ));
    assert!(text.ends_with("NULL\tNULL\tNULL\tNULL\tNULL\tNULL\t0x\n"));
    let result = cli_support::run(&["-vv", "inspect", db.path(), "--query", "Saved"]);
    assert!(result.status.success());
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    for private in [db.path(), "Saved", "NeverRunMe"] {
        assert!(!diagnostics.contains(private));
    }
}

#[test]
fn saved_query_mode_conflicts_and_non_query_names_do_not_fall_back_to_other_inspection() {
    let db = Database::new(fixture::database());
    for options in [
        vec!["--catalog"],
        vec!["--relationships"],
        vec!["--table", "Items"],
        vec!["--indexes"],
        vec!["--properties"],
        vec!["--features"],
        vec!["--complex"],
        vec!["--stored-calculation", "Score"],
        vec!["--complex-links", "Score"],
        vec!["--check"],
        vec!["--constraints"],
    ] {
        let mut args = vec!["inspect", db.path(), "--query", "Saved"];
        args.extend(options);
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(result.stdout.is_empty());
    }
    for name in ["Items", "missing"] {
        let result = cli_support::run(&["inspect", db.path(), "--query", name]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let result = cli_support::run(&["inspect", db.path(), "--query", "Empty"]);
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("Fragment rows\t0\n")
    );
}

#[test]
fn late_system_table_corruption_publishes_no_query_metadata() {
    let mut bytes = fixture::database();
    support::put32(
        &mut bytes,
        fixture::DEFINITION_PAGE * fixture::PAGE_BYTES + 16,
        4,
    );
    let db = Database::new(bytes);
    let result = cli_support::run(&["inspect", db.path(), "--query", "Saved"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

fn select_database() -> Vec<u8> {
    let row = |attribute| Fragment {
        object: Some(fixture::QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    };
    fixture::database_with_query_flags(
        &[
            row(0),
            Fragment {
                flag: Some(1),
                ..row(1)
            },
            Fragment {
                flag: Some(9),
                ..row(3)
            },
            Fragment {
                name1: Some("Items"),
                name2: Some("s"),
                ..row(5)
            },
            Fragment {
                expression: Some("NeverRunMe([s].[ID])"),
                name1: Some("Probe"),
                ..row(6)
            },
            row(255),
        ],
        0x80000000,
    )
}

#[test]
fn saved_select_command_prints_owned_syntax_without_resolving_or_running_calls() {
    for code in [5, 6] {
        let mut bytes = select_database();
        bytes[20] = code;
        let db = Database::new(bytes);
        for prefix in [vec![], vec!["-vv"]] {
            let mut args = prefix;
            args.extend(["query", "saved", "--database", db.path(), "sAvEd"]);
            let result = cli_support::run(&args);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                "SELECT DISTINCTROW NeverRunMe([s].[ID]) AS [Probe], * FROM [Items] AS [s];\n"
            );
            let diagnostics = String::from_utf8(result.stderr).unwrap();
            for private in [db.path(), "sAvEd", "NeverRunMe", "Probe", "Items"] {
                assert!(!diagnostics.contains(private));
            }
        }
    }
}

#[test]
fn saved_select_requires_explicit_arguments_and_rejects_execution_or_write_options() {
    let db = Database::new(select_database());
    for args in [
        vec!["query", "saved"],
        vec!["query", "saved", "Saved"],
        vec!["query", "saved", "--database", db.path()],
        vec![
            "query",
            "saved",
            "--database",
            db.path(),
            "Saved",
            "--param",
            "X=long:1",
        ],
        vec![
            "query",
            "saved",
            "--database",
            db.path(),
            "Saved",
            "--sqlite",
            db.path(),
        ],
        vec![
            "query",
            "saved",
            "--database",
            db.path(),
            "Saved",
            "--index",
            "Items",
            "PK",
        ],
        vec![
            "query",
            "saved",
            "--database",
            db.path(),
            "Saved",
            "--output",
            db.path(),
        ],
    ] {
        let result = cli_support::run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(result.stdout.is_empty());
    }
    let help = cli_support::run(&["query", "saved", "--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("without executing"));
    assert!(text.contains("--database <PATH>"));
    assert!(text.contains("<NAME>"));
}

#[test]
fn saved_select_failures_remain_buffered_and_never_fall_back_to_raw_or_execution_modes() {
    let db = Database::new(select_database());
    for name in ["Items", "missing", "Empty"] {
        let result = cli_support::run(&["query", "saved", "--database", db.path(), name]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
    let opaque = Database::new(fixture::database());
    let result = cli_support::run(&[
        "-vv",
        "query",
        "saved",
        "--database",
        opaque.path(),
        "Saved",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    for private in [opaque.path(), "Saved", "NeverRunMe", "native é"] {
        assert!(!diagnostics.contains(private));
    }
    let mut bytes = select_database();
    support::put32(
        &mut bytes,
        fixture::DEFINITION_PAGE * fixture::PAGE_BYTES + 16,
        7,
    );
    let corrupt = Database::new(bytes);
    let result = cli_support::run(&["query", "saved", "--database", corrupt.path(), "Saved"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn saved_select_command_retains_the_registered_examples_leading_digit_source_name() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/Access_Example_VBA.accdb");
    let before = std::fs::read(&path).unwrap();
    let result = cli_support::run(&[
        "query",
        "saved",
        "--database",
        path.to_str().unwrap(),
        "~sq_c100_Main_Menu_frm~sq_ccboCREWID",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "SELECT DISTINCTROW * FROM [101f_cbo_Crew_Id_qry];\n"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn saved_parameter_declarations_print_without_values_or_prompts_and_unknown_types_fail() {
    for flag in [4, 0] {
        let row = |attribute| Fragment {
            object: Some(fixture::QUERY_ID),
            attribute: Some(attribute),
            order: Some(&[0, 0, 0, 1]),
            ..Default::default()
        };
        let db = Database::new(fixture::database_with_query_flags(
            &[
                Fragment {
                    name1: Some("Prompt?"),
                    flag: Some(flag),
                    ..row(2)
                },
                Fragment {
                    expression: Some("[Prompt?]"),
                    name1: Some("Answer"),
                    ..row(6)
                },
            ],
            0x80000000,
        ));
        let result = cli_support::run(&["-vv", "query", "saved", "--database", db.path(), "Saved"]);
        if flag == 4 {
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                "PARAMETERS [Prompt?] LONG; SELECT [Prompt?] AS [Answer];\n"
            );
        } else {
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
        }
        let diagnostics = String::from_utf8(result.stderr).unwrap();
        for private in [db.path(), "Saved", "Prompt?", "Answer"] {
            assert!(!diagnostics.contains(private));
        }
    }
}
