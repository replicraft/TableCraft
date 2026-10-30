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
fn absent_parent_empty_list_and_stored_null_child_have_distinct_cli_spellings() {
    for code in [2, 5, 6] {
        let database = Database::new(join_fixture::with_null_parent(code));
        for mode in ["--multivalues", "--collections"] {
            let sql = "SELECT a.Id AS Id,a.Score AS L,b.Score AS R FROM Items a \
                       LEFT JOIN Items b ON a.Id=b.Id+1 ORDER BY a.Id";
            let result = run(&["query", "run", sql, "--database", database.path(), mode]);

            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                result.stdout,
                b"Id\tL\tR\n1\t[101, 100, 101, NULL]\tNULL\n\
                  2\t[NULL]\t[101, 100, 101, NULL]\n3\t[]\t[NULL]\n"
            );
            assert!(result.stderr.is_empty());
        }
    }
}

#[test]
fn right_and_nested_outer_joins_keep_absence_through_derived_outputs() {
    let database = Database::new(join_fixture::with_null_parent(6));
    let sql = "SELECT Q.Id,Q.L,Q.R,Q.L AS Again FROM \
               (SELECT b.Id,a.Score AS L,b.Score AS R FROM Items a \
                RIGHT JOIN Items b ON a.Id+1=b.Id) Q ORDER BY Q.Id";
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
    assert_eq!(
        result.stdout,
        b"Id\tL\tR\tAgain\n1\tNULL\t[101, 100, 101, NULL]\tNULL\n\
          2\t[101, 100, 101, NULL]\t[NULL]\t[101, 100, 101, NULL]\n\
          3\t[NULL]\t[]\t[NULL]\n"
    );

    let sql = "SELECT a.Id AS Id,b.Score AS B,c.Score AS C FROM Items a \
               LEFT JOIN (Items b LEFT JOIN Items c ON b.Id+5=c.Id) \
               ON a.Id+5=b.Id ORDER BY a.Id";
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
    assert_eq!(
        result.stdout,
        b"Id\tB\tC\n1\tNULL\tNULL\n2\tNULL\tNULL\n3\tNULL\tNULL\n"
    );
}

#[test]
fn outer_join_eager_errors_and_output_limits_keep_stdout_empty() {
    let database = Database::new(join_fixture::database(6, 0));
    for (sql, flags, reason) in [
        (
            "SELECT TOP 1 a.Score,b.Score FROM Items a LEFT JOIN Items b \
             ON False AND 1/(b.Id-2)>0 WHERE False",
            vec![],
            "division by zero",
        ),
        (
            "SELECT a.Score,b.Score FROM Items a LEFT JOIN Items b ON a.Id+5=b.Id",
            vec!["--output-bytes", "1"],
            "output",
        ),
    ] {
        let mut args = vec![
            "query",
            "run",
            sql,
            "--database",
            database.path(),
            "--multivalues",
        ];
        args.extend(flags);
        let result = run(&args);

        assert!(!result.status.success(), "{sql}");
        assert!(result.stdout.is_empty(), "{sql}");
        assert!(String::from_utf8_lossy(&result.stderr).contains(reason));
    }
}
