mod support;

use rusqlite::{Connection, params};
use std::{fs, path::PathBuf, process::Output};
use support::run;

const INPUT_ROWS: i64 = 1_100;

struct Fixture {
    _directory: tempfile::TempDir,
    path: PathBuf,
    original: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("integral-join.sqlite");
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE Numbers(N INTEGER NOT NULL); \
                 CREATE TABLE Invalid(N INTEGER); \
                 INSERT INTO Invalid VALUES(1),('invalid');",
            )
            .unwrap();
        let transaction = connection.transaction().unwrap();
        {
            let mut insert = transaction
                .prepare("INSERT INTO Numbers VALUES(?)")
                .unwrap();
            for number in 1..=INPUT_ROWS {
                insert.execute(params![number]).unwrap();
            }
        }
        transaction.commit().unwrap();
        connection.close().unwrap();
        let original = fs::read(&path).unwrap();
        Self {
            _directory: directory,
            path,
            original,
        }
    }

    fn query(&self, sql: &str) -> Output {
        run(&[
            "query",
            "run",
            "--sqlite",
            self.path.to_str().unwrap(),
            "--execution-bytes",
            "268435456",
            "--execution-rows",
            "10000",
            "--execution-steps",
            "100000000",
            sql,
        ])
    }

    fn expected(&self, sql: &str) -> Vec<u8> {
        let connection =
            Connection::open_with_flags(&self.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        let mut statement = connection.prepare(sql).unwrap();
        let numbers = statement.query_map([], |row| row.get::<_, i64>(0)).unwrap();
        let mut expected = String::from("N\n");
        for number in numbers {
            expected.push_str(&number.unwrap().to_string());
            expected.push('\n');
        }
        expected.into_bytes()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.original);
    }
}

#[test]
fn sparse_integral_join_results_fit_without_materializing_rejected_pairs() {
    let fixture = Fixture::new();
    for suffix in ["", " WHERE A.N<=3"] {
        let sql = format!("SELECT A.N FROM Numbers A INNER JOIN Numbers B ON A.N=B.N{suffix}");
        let expected = fixture.expected(&sql);
        let output = fixture.query(&sql);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn rejected_pairs_do_not_hide_invalid_selected_source_carriers() {
    let fixture = Fixture::new();
    let output =
        fixture.query("SELECT A.N FROM Numbers A INNER JOIN Invalid B ON A.N=B.N WHERE False");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
