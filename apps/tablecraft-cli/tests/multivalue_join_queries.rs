use tablecraft_testkit::access::mutation_fixture as base;
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
use tablecraft_testkit::access as support;

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
fn native_inner_and_cross_join_lists_keep_parent_rows_nulls_and_duplicate_members() {
    for code in [2, 5, 6] {
        let database = Database::new(join_fixture::database(code, 0));
        for (sql, expected) in [
            (
                "SELECT a.Id AS A,b.Id AS B,a.Score AS L,b.Score AS R FROM Items a \
                 INNER JOIN Items b ON a.Id<b.Id ORDER BY a.Id,b.Id",
                "A\tB\tL\tR\n1\t2\t[101, 100, 101, NULL]\t[200]\n\
                 1\t3\t[101, 100, 101, NULL]\t[]\n2\t3\t[200]\t[]\n",
            ),
            (
                "SELECT a.Id AS A,b.Id AS B,a.Score AS L,b.Score AS R FROM Items a \
                 ,Items b WHERE a.Score.Value=100 AND a.Score.Value=101 \
                 AND b.Score.Value=200",
                "A\tB\tL\tR\n1\t2\t[101, 100, 101, NULL]\t[200]\n",
            ),
        ] {
            let result = run(&[
                "query",
                "run",
                sql,
                "--database",
                database.path(),
                "--multivalues",
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, expected.as_bytes());
            assert!(result.stderr.is_empty());
        }

        let nullable = Database::new(join_fixture::with_null_parent(code));
        let result = run(&[
            "query",
            "run",
            "SELECT a.Id AS A,b.Id AS B,a.Score AS L,b.Score AS R FROM Items a \
             INNER JOIN Items b ON a.Id=b.Id WHERE a.Id>=2 ORDER BY a.Id",
            "--database",
            nullable.path(),
            "--multivalues",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"A\tB\tL\tR\n2\t2\t[NULL]\t[NULL]\n3\t3\t[]\t[]\n"
        );
    }
}

#[test]
fn explicit_link_and_external_join_routes_return_their_own_complete_lists() {
    let root = Database::new(linked_fixture::database(
        6,
        Some("opaque"),
        Some("Items"),
        None,
    ));
    let linked = Database::new(join_fixture::database(6, 0));
    let external = Database::new(join_fixture::database(6, 1000));
    let result = run(&[
        "query",
        "run",
        "SELECT a.Id AS A,b.Id AS B,a.Score AS L,b.Score AS R FROM Linked a \
         INNER JOIN [authorized].Items b ON a.Id<b.Id ORDER BY a.Id,b.Id",
        "--database",
        root.path(),
        "--multivalues",
        "--link",
        "Linked",
        linked.path(),
        "--external",
        "authorized",
        external.path(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"A\tB\tL\tR\n1\t2\t[101, 100, 101, NULL]\t[1200]\n\
        1\t3\t[101, 100, 101, NULL]\t[]\n2\t3\t[200]\t[]\n"
    );
}

#[test]
fn selected_corruption_eager_errors_and_limits_publish_no_join_prefix() {
    let valid = Database::new(join_fixture::database(6, 0));
    for (sql, flags, reason) in [
        (
            "SELECT a.Id,a.Score,a.Id Mod (b.Id-2) AS Boom FROM Items a ,Items b",
            vec![],
            "division by zero",
        ),
        (
            "SELECT a.Score,b.Score FROM Items a ,Items b",
            vec!["--output-bytes", "1"],
            "output",
        ),
        (
            "SELECT a.Score,b.Score FROM Items a ,Items b",
            vec!["--native-rows", "1"],
            "resource",
        ),
        (
            "SELECT a.Score,b.Score FROM Items a LEFT JOIN Items b ON a.Score=b.Score",
            vec![],
            "not implemented",
        ),
    ] {
        let mut arguments = vec![
            "query",
            "run",
            sql,
            "--database",
            valid.path(),
            "--multivalues",
        ];
        arguments.extend(flags);
        let result = run(&arguments);
        assert!(!result.status.success(), "{sql}");
        assert!(result.stdout.is_empty(), "{sql}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(reason),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let mut bad = join_fixture::database(6, 0);
    bad[CHILD_DATA_PAGE * PAGE_BYTES] = 0;
    let corrupt = Database::new(bad);
    let result = run(&[
        "query",
        "run",
        "SELECT a.Score,b.Score FROM Items a ,Items b",
        "--database",
        corrupt.path(),
        "--multivalues",
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!result.stderr.is_empty());
}
