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
fn native_complete_lists_repeat_for_displayed_duplicates_nulls_and_empty_placeholders() {
    for code in [2, 5, 6] {
        let database = Database::new(join_fixture::with_null_parent(code));
        for mode in ["--multivalues", "--collections"] {
            let sql = "SELECT Id,Score,Score.Value AS One FROM Items";
            let result = run(&["query", "run", sql, "--database", database.path(), mode]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                result.stdout,
                b"Id\tScore\tOne\n\
                1\t[101, 100, 101, NULL]\t101\n\
                1\t[101, 100, 101, NULL]\t100\n\
                1\t[101, 100, 101, NULL]\t101\n\
                1\t[101, 100, 101, NULL]\tNULL\n\
                2\t[NULL]\tNULL\n3\t[]\tNULL\n"
            );
            assert!(result.stderr.is_empty());
        }
        let sql = "SELECT Score,Score.Value AS One FROM Items WHERE Score.Value=101";
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
            b"Score\tOne\n\
            [101, 100, 101, NULL]\t101\n[101, 100, 101, NULL]\t101\n"
        );
    }
}

#[test]
fn joined_hidden_criteria_and_outer_absence_keep_complete_display_lists() {
    let database = Database::new(join_fixture::database(6, 0));
    let sql = "SELECT a.Score AS Whole,a.Score.Value AS One,b.Score AS Peer FROM Items a \
               INNER JOIN Items b ON a.Id<b.Id WHERE b.Score.Value=200";
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
    let mut expected = String::from("Whole\tOne\tPeer\n");
    for member in ["101", "100", "101", "NULL"] {
        expected.push_str(&format!("[101, 100, 101, NULL]\t{member}\t[200]\n"));
    }
    assert_eq!(result.stdout, expected.as_bytes());

    let sql = "SELECT a.Id AS Id,b.Score AS Whole,b.Score.Value AS One FROM Items a \
               LEFT JOIN Items b ON a.Id=b.Id+1 WHERE a.Id=1";
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
    assert_eq!(result.stdout, b"Id\tWhole\tOne\n1\tNULL\tNULL\n");
}

#[test]
fn derived_sort_top_and_eager_failures_keep_the_current_buffered_contract() {
    let database = Database::new(join_fixture::database(6, 0));
    let sql = "SELECT TOP 2 Whole,One FROM \
               (SELECT Score AS Whole,Score.Value AS One FROM Items) Q ORDER BY One DESC";
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
        b"Whole\tOne\n[200]\t200\n[101, 100, 101, NULL]\t101\n\
          [101, 100, 101, NULL]\t101\n"
    );

    for (sql, flags, reason) in [
        (
            "SELECT TOP 1 Score,10 \\ (Score.Value-200) AS Boom FROM Items",
            vec![],
            "division by zero",
        ),
        (
            "SELECT Score,Score.Value FROM Items",
            vec!["--output-bytes", "1"],
            "output",
        ),
        (
            "SELECT DISTINCT Score,Score.Value FROM Items",
            vec![],
            "not implemented",
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
