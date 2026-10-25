//! Explicit fresh query views leave native caches and original files intact.
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod agile_fixture;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "support/mod.rs"]
mod cli_support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use cli_support::run;
use std::{fs, path::PathBuf, process::Output};

const FORMAT_OFFSET: usize = 20;
const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;
const FIRST_ROW_SLOT: usize = 14;
const CACHE_WRAPPER_OFFSET: usize = 6;

struct Database {
    _directory: tempfile::TempDir,
    path: PathBuf,
    bytes: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recomputed.accdb");
        fs::write(&path, &bytes).unwrap();
        Self {
            _directory: directory,
            path,
            bytes,
        }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }

    fn query(&self, sql: &str, columns: &[&str], extra: &[&str]) -> Output {
        let mut arguments = vec![
            "--format",
            "tsv",
            "query",
            "run",
            sql,
            "--database",
            self.path(),
        ];
        for column in columns {
            arguments.extend(["--recomputed-column", "Fresh", "Items", column]);
        }
        arguments.extend_from_slice(extra);
        run(&arguments)
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.bytes);
    }
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

fn failure(output: Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn explicit_views_filter_and_group_fresh_values_without_replacing_stored_caches() {
    for profile in [5, 6] {
        let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
        bytes[FORMAT_OFFSET] = profile;
        let database = Database::new(bytes);
        assert_eq!(
            success(database.query(
                "SELECT Id,Score FROM Fresh WHERE Score>2",
                &["Id", "Score"],
                &[]
            )),
            b"Id\tScore\n2\t4\n3\t6\n"
        );
        assert_eq!(
            success(database.query(
                "SELECT Score,Count(*) AS N FROM Fresh GROUP BY Score",
                &["Score"],
                &[]
            )),
            b"Score\tN\n2\t1\n4\t1\n6\t1\n"
        );
        assert_eq!(
            success(database.query("SELECT Score FROM Items", &[], &["--stored-calculations"])),
            b"Score\n999\nNULL\n41\n"
        );
        failure(database.query("SELECT Score FROM Items", &[], &[]));
    }
}

#[test]
fn views_compose_with_joins_sets_and_domain_queries_in_the_existing_executor() {
    let database = Database::new(calculated_fixture::database(&[Some(999), None, Some(41)]));
    for (sql, expected) in [
        (
            "SELECT F.Id,F.Score,I.Id AS Original FROM Fresh F INNER JOIN Items I ON F.Id=I.Id",
            b"Id\tScore\tOriginal\n1\t2\t1\n2\t4\t2\n3\t6\t3\n".as_slice(),
        ),
        (
            "SELECT Score AS S FROM Fresh UNION SELECT Id FROM Items ORDER BY S",
            b"S\n1\n2\n3\n4\n6\n".as_slice(),
        ),
        (
            "SELECT DSum('Score','Fresh') AS Total",
            b"Total\n12\n".as_slice(),
        ),
    ] {
        assert_eq!(
            success(database.query(sql, &["Id", "Score"], &[])),
            expected,
            "{sql}"
        );
    }
}

#[test]
fn natural_text_and_widened_integer_results_do_not_inherit_stored_long_constraints() {
    let text = Database::new(calculated_fixture::database_with_expression(
        &[Some(999)],
        "'row=' & [Id]",
    ));
    assert_eq!(
        success(text.query(
            "SELECT Score,VarType(Score) AS T FROM Fresh",
            &["Score"],
            &["--number-format", "invariant"]
        )),
        b"Score\tT\n\"\"\"row=1\"\"\"\t8\n"
    );

    let integer = Database::new(calculated_fixture::database_with_expression(
        &[Some(999)],
        "2147483647+[Id]",
    ));
    let sql = "SELECT Score,VarType(Score) AS T FROM Fresh";
    assert_eq!(
        success(integer.query(sql, &["Score"], &[])),
        b"Score\tT\n2147483648\t20\n"
    );
    failure(integer.query(sql, &["Score"], &["--declared-arithmetic"]));
}

#[test]
fn unrelated_formulas_remain_inert_but_every_selected_view_expression_is_eager() {
    let unsupported = Database::new(calculated_fixture::database_with_expression(
        &[Some(999)],
        "Now()",
    ));
    assert_eq!(
        success(unsupported.query("SELECT Id FROM Fresh", &["Id"], &[])),
        b"Id\n1\n"
    );
    failure(unsupported.query("SELECT Id FROM Fresh WHERE False", &["Id", "Score"], &[]));

    let late = Database::new(calculated_fixture::database_with_expression(
        &[Some(999), Some(999)],
        "1/([Id]-2)",
    ));
    for sql in [
        "SELECT Id FROM Fresh",
        "SELECT Id FROM Fresh WHERE False",
        "SELECT TOP 1 Id FROM Fresh",
    ] {
        let output = late.query(sql, &["Id", "Score"], &[]);
        assert!(String::from_utf8_lossy(&output.stderr).contains("division by zero"));
        failure(output);
    }
    assert_eq!(
        success(late.query("SELECT Id FROM Fresh", &["Id"], &[])),
        b"Id\n1\n2\n"
    );
}

#[test]
fn malformed_unselected_cache_is_pruned_from_fresh_expression_leaves() {
    let mut bytes = calculated_fixture::database(&[Some(999)]);
    let page = DATA_PAGE * PAGE_BYTES;
    let row = u16::from_le_bytes(
        bytes[page + FIRST_ROW_SLOT..page + FIRST_ROW_SLOT + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    bytes[page + row + CACHE_WRAPPER_OFFSET] = 1;
    let database = Database::new(bytes);
    assert_eq!(
        success(database.query("SELECT Score FROM Fresh", &["Score"], &[])),
        b"Score\n2\n"
    );
    failure(database.query("SELECT Score FROM Items", &[], &["--stored-calculations"]));
}

#[test]
fn selected_views_preserve_password_authentication_and_original_ciphertext() {
    let plain = calculated_fixture::database(&[Some(999), None]);
    let password = "public recomputed password";
    let database = Database::new(agile_fixture::encrypted_image(&plain, 6, password));
    for extra in [vec![], vec!["-p", "incorrect"]] {
        failure(database.query("SELECT Score FROM Fresh", &["Score"], &extra));
    }
    assert_eq!(
        success(database.query("SELECT Score FROM Fresh", &["Score"], &["-p", password])),
        b"Score\n2\n4\n"
    );
}

#[test]
fn source_query_and_output_limits_remain_independent_and_buffered() {
    let database = Database::new(calculated_fixture::database(&[Some(999), None, Some(41)]));
    for controls in [
        ["--native-bytes", "1"],
        ["--native-rows", "1"],
        ["--native-steps", "1"],
        ["--execution-bytes", "1"],
        ["--execution-rows", "1"],
        ["--execution-steps", "1"],
        ["--output-bytes", "1"],
    ] {
        failure(database.query("SELECT Score FROM Fresh", &["Score"], &controls));
    }
}

#[test]
fn duplicate_columns_existing_names_and_inconsistent_tables_are_refused() {
    let database = Database::new(calculated_fixture::database(&[Some(999)]));
    failure(database.query("SELECT Id FROM Fresh", &["Id", "id"], &[]));
    failure(database.query(
        "SELECT Id FROM Fresh",
        &["Id"],
        &["--recomputed-column", "Fresh", "Other", "Id"],
    ));
    let output = run(&[
        "query",
        "run",
        "SELECT Id FROM Items",
        "--database",
        database.path(),
        "--recomputed-column",
        "items",
        "Items",
        "Id",
    ]);
    failure(output);
}

#[test]
fn help_and_argument_refusals_expose_explicit_local_views() {
    let help = success(run(&["query", "run", "--help"]));
    let help = String::from_utf8(help).unwrap();
    assert!(help.contains("--recomputed-column <VIEW> <TABLE> <COLUMN>"));

    for extra in [
        vec!["--stream", "--index", "Items", "Missing"],
        vec!["--link", "Linked", "unused.accdb"],
        vec!["--external", "remote", "unused.accdb"],
    ] {
        let mut arguments = vec![
            "query",
            "run",
            "SELECT Id FROM Fresh",
            "--database",
            "unopened.accdb",
            "--recomputed-column",
            "Fresh",
            "Items",
            "Id",
        ];
        arguments.extend(extra);
        let output = run(&arguments);
        assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
        failure(output);
    }
}

#[test]
fn fresh_view_streams_preserve_natural_values_and_eager_formula_failures() {
    let database = Database::new(calculated_fixture::database(&[Some(999), None, Some(41)]));
    assert_eq!(
        success(database.query(
            "SELECT Id,Score FROM Fresh WHERE Score>2",
            &["Id", "Score"],
            &["--stream"]
        )),
        b"Id\tScore\n2\t4\n3\t6\n"
    );
    assert_eq!(
        success(database.query(
            "SELECT Score,Count(*) AS N FROM Fresh GROUP BY Score",
            &["Score"],
            &["--stream"]
        )),
        b"Score\tN\n2\t1\n4\t1\n6\t1\n"
    );
    let text = Database::new(calculated_fixture::database_with_expression(
        &[Some(999)],
        "'row=' & [Id]",
    ));
    assert_eq!(
        success(text.query(
            "SELECT Score,VarType(Score) AS T FROM Fresh",
            &["Score"],
            &["--stream", "--number-format", "invariant"]
        )),
        b"Score\tT\n\"\"\"row=1\"\"\"\t8\n"
    );

    let late = Database::new(calculated_fixture::database_with_expression(
        &[Some(999), Some(999)],
        "1/([Id]-2)",
    ));
    let failed = late.query(
        "SELECT TOP 1 Id FROM Fresh",
        &["Id", "Score"],
        &["--stream", "--batch-rows", "1"],
    );
    assert_eq!(failed.status.code(), Some(2));
    assert_eq!(failed.stdout, b"Id\n1\n");
    assert!(String::from_utf8_lossy(&failed.stderr).contains("division by zero"));
}
