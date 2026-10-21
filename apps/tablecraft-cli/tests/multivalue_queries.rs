#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[path = "support/mod.rs"]
mod cli_support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
use cli_support::run;
use std::{
    fs,
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
            "tablecraft-multivalue-query-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
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
        assert_eq!(fs::read(&self.path).unwrap(), self.bytes);
        fs::remove_file(&self.path).unwrap();
    }
}
#[test]
fn explicit_multivalue_queries_flatten_all_priority_ace_profiles_without_changing_bytes() {
    for code in [2, 5, 6] {
        let mut bytes = links_fixture::populated();
        bytes[20] = code;
        let database = Database::new(bytes);
        let result = run(&[
            "query",
            "run",
            "SELECT Id,Score.Value AS V FROM Items ORDER BY Id,V",
            "--database",
            database.path(),
            "--multivalues",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Id\tV\n1\t100\n1\t101\n2\t200\n3\tNULL\n");
        assert!(result.stderr.is_empty());
        let result = run(&[
            "query",
            "run",
            "SELECT Id,Count(Score.Value) AS N FROM Items GROUP BY Id ORDER BY Id",
            "--database",
            database.path(),
            "--multivalues",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Id\tN\n1\t2\n2\t1\n3\t0\n");
    }
}
#[test]
fn invalid_scope_ambiguous_member_sort_and_selected_index_never_publish_partial_results() {
    for args in [
        vec!["query", "run", "SELECT 1", "--multivalues"],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--sqlite",
            "missing.sqlite",
            "--multivalues",
        ],
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let database = Database::new(links_fixture::populated());
    for (sql, options) in [
        ("SELECT Score.Value FROM Items", vec![]),
        (
            "SELECT Id FROM Items WHERE Score.Value=100 ORDER BY Score.Value",
            vec!["--multivalues"],
        ),
        (
            "SELECT Id,Score.Value FROM Items",
            vec!["--multivalues", "--index", "Items", "Missing"],
        ),
        ("SELECT * FROM Items", vec!["--multivalues"]),
    ] {
        let mut args = vec!["query", "run", sql, "--database", database.path()];
        args.extend(options);
        let output = run(&args);
        assert!(!output.status.success(), "{sql}");
        assert!(output.stdout.is_empty(), "{sql}");
    }
}

#[test]
fn parent_criteria_preserve_parent_counts_and_match_multiple_stored_members() {
    for code in [2, 5, 6] {
        let mut bytes = links_fixture::populated();
        bytes[20] = code;
        let database = Database::new(bytes);
        for (sql, expected) in [
            (
                "SELECT Id FROM Items WHERE Score.Value=100 AND Score.Value=101 ORDER BY Id",
                &b"Id\n1\n"[..],
            ),
            (
                "SELECT Id FROM Items WHERE NOT Score.Value=100 ORDER BY Id",
                &b"Id\n2\n3\n"[..],
            ),
            (
                "SELECT Count(*) AS N FROM Items WHERE Score.Value IN (100,101)",
                &b"N\n1\n"[..],
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
            assert_eq!(result.stdout, expected);
        }
    }
}

#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod attachment_fixture;
#[test]
fn decoded_attachment_queries_require_explicit_opt_in_and_buffer_late_errors() {
    let empty_file = [
        0, 0, 0, 0, 14, 0, 0, 0, 14, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0,
    ];
    for code in [2, 5, 6] {
        let mut bytes = attachment_fixture::with_payloads(attachment_fixture::PAYLOAD, &empty_file);
        bytes[20] = code;
        let database = Database::new(bytes.clone());
        let sql = "SELECT Id,Score.FileData AS Data FROM Items";
        let result = run(&[
            "query",
            "run",
            sql,
            "--database",
            database.path(),
            "--collections",
            "--decoded-attachments",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            result.stdout,
            b"Id\tData\n1\t0x000102ff\n1\tNULL\n2\t0x\n3\tNULL\n"
        );
        let hidden = run(&[
            "query",
            "run",
            "SELECT Id,Score.FileData AS Data FROM Items WHERE Score.FileFlags<0",
            "--database",
            database.path(),
            "--collections",
            "--decoded-attachments",
        ]);
        assert!(
            hidden.status.success(),
            "{}",
            String::from_utf8_lossy(&hidden.stderr)
        );
        assert_eq!(hidden.stdout, b"Id\tData\n1\t0x000102ff\n2\t0x\n");
        let joined = run(&[
            "query",
            "run",
            "SELECT A.Id AS Parent,B.Id AS Other FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE A.Score.FileFlags<0 AND B.Score.FileData IS NULL",
            "--database",
            database.path(),
            "--collections",
            "--decoded-attachments",
        ]);
        assert!(
            joined.status.success(),
            "{}",
            String::from_utf8_lossy(&joined.stderr)
        );
        assert_eq!(joined.stdout, b"Parent\tOther\n1\t1\n");
        let mixed = run(&[
            "query",
            "run",
            "SELECT A.Id AS Parent,A.Score.FileData AS Data,B.Id AS Other FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE B.Score.FileData IS NULL ORDER BY A.Id",
            "--database",
            database.path(),
            "--collections",
            "--decoded-attachments",
        ]);
        assert!(
            mixed.status.success(),
            "{}",
            String::from_utf8_lossy(&mixed.stderr)
        );
        assert_eq!(
            mixed.stdout,
            b"Parent\tData\tOther\n1\t0x000102ff\t1\n1\tNULL\t1\n"
        );
        for sql in [
            "TRANSFORM Count(*) SELECT Id FROM Items WHERE Score.FileData IS NULL GROUP BY Id PIVOT Id",
            "TRANSFORM Count(A.Score.FileData) SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE B.Score.FileData IS NULL GROUP BY A.Id PIVOT A.Id IN (1,2,3)",
        ] {
            let result = run(&[
                "query",
                "run",
                sql,
                "--database",
                database.path(),
                "--collections",
                "--decoded-attachments",
            ]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, b"Id\t1\t2\t3\n1\t1\tNULL\tNULL\n");
        }
        let nested = run(&[
            "query",
            "run",
            "SELECT A.Id AS Parent,B.Id AS Other FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE EXISTS(SELECT 1 WHERE A.Score.FileData IS NULL)",
            "--database",
            database.path(),
            "--collections",
            "--decoded-attachments",
        ]);
        assert!(
            nested.status.success(),
            "{}",
            String::from_utf8_lossy(&nested.stderr)
        );
        assert_eq!(nested.stdout, b"Parent\tOther\n1\t1\n");
        let joined_children = run(&[
            "query",
            "run",
            "SELECT A.Id AS Parent,B.Id AS Other FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id AND A.Score.FileData IS NOT NULL ORDER BY A.Id",
            "--database",
            database.path(),
            "--collections",
            "--decoded-attachments",
        ]);
        assert!(
            joined_children.status.success(),
            "{}",
            String::from_utf8_lossy(&joined_children.stderr)
        );
        assert_eq!(joined_children.stdout, b"Parent\tOther\n1\t1\n2\t2\n");
        let aligned_hidden = run(&[
            "query",
            "run",
            "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Score.FileData IS NULL AND Score.FileFlags<0)",
            "--database",
            database.path(),
            "--collections",
            "--decoded-attachments",
        ]);
        assert!(
            aligned_hidden.status.success(),
            "{}",
            String::from_utf8_lossy(&aligned_hidden.stderr)
        );
        assert_eq!(aligned_hidden.stdout, b"Id\n");
        for options in [
            vec!["--collections"],
            vec!["--decoded-attachments"],
            vec!["--multivalues", "--decoded-attachments"],
        ] {
            let mut command = vec!["query", "run", sql, "--database", database.path()];
            command.extend(options);
            let result = run(&command);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
        assert_eq!(fs::read(database.path()).unwrap(), bytes);
        let invalid = Database::new(attachment_fixture::database());
        for sql in [
            "SELECT Score.FileData FROM Items",
            "SELECT TOP 1 Score.FileData FROM Items",
            "SELECT Score.FileData FROM Items WHERE Id=999",
            "SELECT TOP 1 Score.FileName FROM Items WHERE Score.FileData IS NOT NULL",
            "SELECT TOP 1 A.Id FROM Items AS A,Items AS B WHERE A.Score.FileData IS NULL",
            "SELECT TOP 1 A.Score.FileName FROM Items AS A,Items AS B WHERE False AND B.Score.FileData IS NULL",
            "TRANSFORM Count(*) SELECT TOP 1 Id FROM Items WHERE Score.FileData IS NULL GROUP BY Id PIVOT Id",
            "SELECT TOP 1 Id FROM Items WHERE EXISTS(SELECT 1 WHERE Score.FileData IS NOT NULL)",
            "SELECT TOP 1 A.Id FROM Items AS A INNER JOIN Items AS B ON False AND A.Score.FileData IS NOT NULL",
            "SELECT TOP 1 Id FROM Items WHERE EXISTS(SELECT 1 WHERE Score.FileData IS NOT NULL AND Score.FileFlags<0)",
        ] {
            let result = run(&[
                "query",
                "run",
                sql,
                "--database",
                invalid.path(),
                "--collections",
                "--decoded-attachments",
            ]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
}
#[test]
fn collections_flag_exposes_aligned_attachment_metadata_and_preserves_value_queries() {
    for code in [2, 5, 6] {
        let mut bytes = attachment_fixture::database();
        bytes[20] = code;
        let database = Database::new(bytes);
        let result = run(&[
            "query",
            "run",
            "SELECT Id,Count(Score.FileType) AS N FROM Items GROUP BY Id ORDER BY Id",
            "--database",
            database.path(),
            "--collections",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Id\tN\n1\t1\n2\t1\n3\t0\n");
        for sql in ["SELECT Score.FileData FROM Items", "SELECT * FROM Items"] {
            let result = run(&[
                "query",
                "run",
                sql,
                "--database",
                database.path(),
                "--collections",
            ]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
    let database = Database::new(links_fixture::populated());
    let result = run(&[
        "query",
        "run",
        "SELECT Score.Value AS V FROM Items",
        "--database",
        database.path(),
        "--collections",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"V\n100\n101\n200\nNULL\n");
    for args in [
        vec!["query", "run", "SELECT 1", "--collections"],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--collections",
            "--sqlite",
            "missing.sqlite",
        ],
        vec![
            "query",
            "run",
            "SELECT 1",
            "--database",
            database.path(),
            "--collections",
            "--multivalues",
        ],
    ] {
        let result = run(&args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}
