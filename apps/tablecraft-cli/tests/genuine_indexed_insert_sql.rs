#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_testkit::fixtures::access::genuine_indexed_insert as sources;

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn insert(path: &Path, bundle: &Path, sql: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "journal-insert", sql])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(bundle)
        .output()
        .unwrap()
}

#[test]
#[ignore = "requires all six source-first indexed INSERT views and SDK36 qualification"]
fn cli_successive_indexed_inserts_match_literal_single_batch_images_and_rows() {
    let mut commits = 0;
    let mut reads = 0;
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();

        for (sql, expected, rows) in [
            (
                "INSERT INTO Items ([Id], [Score], [Guard]) VALUES (4, 17, 3)",
                &case.single,
                "Id\tScore\tGuard\n1\t-1\t0\n2\t0\t1\n3\t1\t2\n4\t17\t3\n",
            ),
            (
                "INSERT INTO Items ([Guard], [Id], [Score]) VALUES (3, 5, 18)",
                &case.batch,
                "Id\tScore\tGuard\n1\t-1\t0\n2\t0\t1\n3\t1\t2\n4\t17\t3\n5\t18\t3\n",
            ),
        ] {
            let output = insert(&path, &bundle, sql);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(
                output.stdout,
                concat!(
                    "Transaction\tCommitted\n",
                    "Inserted rows\t1\n",
                    "Journal namespace\tRetired\n",
                    "Retirement guarantee\tCaptured-parent barrier\n",
                    "Journal cleanup\tRemoved\n",
                )
                .as_bytes()
            );
            assert_eq!(fs::read(&path).unwrap(), *expected);
            assert!(!bundle.exists());
            commits += 1;

            let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "run", "SELECT [Id], [Score], [Guard] FROM Items"])
                .arg("--database")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                read.status.success(),
                "{}",
                String::from_utf8_lossy(&read.stderr)
            );
            assert!(read.stderr.is_empty());
            assert_eq!(read.stdout, rows.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), *expected);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            reads += 1;
        }

        let refused = insert(
            &path,
            &bundle,
            "INSERT INTO Items ([Id], [Score], [Guard]) VALUES (4, 99, 3)",
        );
        assert_eq!(refused.status.code(), Some(2));
        assert!(refused.stdout.is_empty());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("duplicate non-NULL key"));
        assert_eq!(fs::read(&path).unwrap(), case.batch);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    assert_eq!((commits, reads), (4, 4));
}
