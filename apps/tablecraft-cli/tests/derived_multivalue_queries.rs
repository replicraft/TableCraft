#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[path = "support/mod.rs"]
mod cli_support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../crates/backends/tablecraft-access/tests/multivalue_join_results/fixture.rs"]
mod join_fixture;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use cli_support::run;
use std::{fs, io::Write, path::PathBuf};
use tempfile::TempDir;

const PAGE_BYTES: usize = 4096;
const CHILD_DATA_PAGE: usize = 22;

struct Database {
    directory: TempDir,
    path: PathBuf,
    original: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let directory = TempDir::new().unwrap();

        let path = directory.path().join("original.accdb");
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        Self {
            directory,
            path,
            original: bytes,
        }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        assert!(self.directory.path().is_dir());
        assert_eq!(fs::read(&self.path).unwrap(), self.original);
    }
}

#[test]
fn native_derived_members_render_full_lists_duplicates_nulls_and_empty_parents() {
    for code in [2, 5, 6] {
        let database = Database::new(join_fixture::with_null_parent(code));
        for mode in ["--multivalues", "--collections"] {
            let result = run(&[
                "query",
                "run",
                "SELECT Q.Id,Q.Whole,Q.Whole.Value AS One FROM \
                 (SELECT Id,Score AS Whole FROM Items) Q",
                "--database",
                database.path(),
                mode,
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                result.stdout,
                b"Id\tWhole\tOne\n\
                1\t[101, 100, 101, NULL]\t101\n\
                1\t[101, 100, 101, NULL]\t100\n\
                1\t[101, 100, 101, NULL]\t101\n\
                1\t[101, 100, 101, NULL]\tNULL\n\
                2\t[NULL]\tNULL\n3\t[]\tNULL\n"
            );
            assert!(result.stderr.is_empty());
        }
    }
}

#[test]
fn derived_absent_parent_and_authorized_external_members_remain_distinct() {
    let database = Database::new(join_fixture::with_null_parent(6));
    let result = run(&[
        "query",
        "run",
        "SELECT Q.Id,Q.Whole,Q.Whole.Value AS One FROM \
         (SELECT a.Id,b.Score AS Whole FROM Items a \
          LEFT JOIN Items b ON a.Id=b.Id+1) Q",
        "--database",
        database.path(),
        "--multivalues",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"Id\tWhole\tOne\n1\tNULL\tNULL\n\
        2\t[101, 100, 101, NULL]\t101\n\
        2\t[101, 100, 101, NULL]\t100\n\
        2\t[101, 100, 101, NULL]\t101\n\
        2\t[101, 100, 101, NULL]\tNULL\n3\t[NULL]\tNULL\n"
    );

    let target = Database::new(join_fixture::database(6, 1000));
    let result = run(&[
        "query",
        "run",
        "SELECT Q.Whole,Q.Whole.Value AS One FROM \
         (SELECT Score AS Whole FROM [authorized].Items WHERE Id=2) Q",
        "--database",
        database.path(),
        "--multivalues",
        "--external",
        "authorized",
        target.path(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Whole\tOne\n[1200]\t1200\n");
}

#[test]
fn derived_eager_failures_corrupt_children_and_limits_publish_no_stdout_prefix() {
    let valid = Database::new(join_fixture::database(6, 0));
    for (sql, flags, reason) in [
        (
            "SELECT Q.Whole,Q.Whole.Value FROM \
             (SELECT Score AS Whole,Id Mod (Id-2) AS Boom FROM Items) Q",
            vec![],
            "division by zero",
        ),
        (
            "SELECT Q.Whole,Q.Whole.Value FROM (SELECT Score AS Whole FROM Items) Q",
            vec!["--output-bytes", "1"],
            "output",
        ),
        (
            "SELECT Q.Whole,Q.Whole.Value FROM (SELECT Score AS Whole FROM Items) Q",
            vec!["--native-rows", "1"],
            "resource",
        ),
    ] {
        let mut args = vec![
            "query",
            "run",
            sql,
            "--database",
            valid.path(),
            "--multivalues",
        ];
        args.extend(flags);
        let result = run(&args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(reason),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let mut bytes = join_fixture::database(6, 0);
    bytes[CHILD_DATA_PAGE * PAGE_BYTES] = 0;
    let corrupt = Database::new(bytes);
    let result = run(&[
        "query",
        "run",
        "SELECT Q.Whole,Q.Whole.Value FROM (SELECT Score AS Whole FROM Items) Q",
        "--database",
        corrupt.path(),
        "--multivalues",
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!result.stderr.is_empty());
}
