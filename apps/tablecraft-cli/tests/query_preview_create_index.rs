//! Real CLI publication of a source-first, independently expected new index.
#[path = "../../../crates/backends/tablecraft-access/tests/create_secondary_index/source.rs"]
mod source;

#[path = "query_preview_create_index/guards.rs"]
mod guards;

use source::{same_bytes, sql};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_testkit::access::secondary_index as model;

fn run(database: &Path, output: &Path, statement: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-create-index", statement, "--database"])
        .arg(database)
        .arg("--output")
        .arg(output)
        .output()
        .unwrap()
}

#[test]
fn qualify_create_secondary_index_cli_sources_before_consumer() {
    source::qualify_all();
}

#[test]
fn real_cli_creates_sixteen_exact_copies_and_preserves_occupied_namespaces() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for code in [5, 6] {
        for population in model::POPULATIONS {
            for descending in [false, true] {
                let case = model::source(code, population, descending);
                let database = directory.path().join(format!("source-{count}.accdb"));
                let output = directory.path().join(format!("copy-{count}.accdb"));
                fs::write(&database, &case.before).unwrap();
                let result = run(&database, &output, &sql(&case));
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert!(result.stderr.is_empty());
                let expected_report = format!(
                    "Engineering copy\tExperimental; data macros and native Access saves unqualified\nCreated index\t{}\nLogical index number\t4\nPhysical index number\t2\nIndexed source rows\t{}\n",
                    case.name,
                    case.rows.len(),
                );
                assert_eq!(String::from_utf8(result.stdout).unwrap(), expected_report);
                same_bytes(&fs::read(&output).unwrap(), &case.expected);
                same_bytes(&fs::read(&database).unwrap(), &case.before);
                source::qualify(&fs::read(&output).unwrap(), &case, true);

                let occupied = run(&database, &output, &sql(&case));
                assert!(!occupied.status.success());
                assert!(occupied.stdout.is_empty());
                same_bytes(&fs::read(&output).unwrap(), &case.expected);
                same_bytes(&fs::read(&database).unwrap(), &case.before);
                count += 1;
            }
        }
    }
    assert_eq!(count, 16);
}

#[test]
fn native_profile_errors_and_source_alias_never_publish_a_copy() {
    let directory = tempfile::tempdir().unwrap();
    let case = model::source(6, model::Population::Distinct, false);
    let database = directory.path().join("source.accdb");
    fs::write(&database, &case.before).unwrap();

    for (position, statement) in [
        "CREATE INDEX [PrimaryAlias] ON [Items] ([Score])",
        "CREATE INDEX [NewLookup] ON [Items] ([Id])",
        "CREATE INDEX [NewLookup] ON [Items] ([Absent])",
        "CREATE INDEX [1Bad] ON [Items] ([Score])",
        "CREATE INDEX [NewLookup] ON [Absent] ([Score])",
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused-{position}.accdb"));
        let result = run(&database, &output, statement);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("engineering index preparation failed")
        );
        assert!(!output.exists());
        same_bytes(&fs::read(&database).unwrap(), &case.before);
    }

    let alias = run(&database, &database, &sql(&case));
    assert!(!alias.status.success());
    assert!(alias.stdout.is_empty());
    same_bytes(&fs::read(&database).unwrap(), &case.before);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}
