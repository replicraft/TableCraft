#[path = "support/mod.rs"]
mod cli_support;
#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod crypto;
use tablecraft_testkit::access::mutation_fixture as fixture;

use cli_support::run;
use std::fs;
use tablecraft_io::{ExclusiveFile, SharedFile};

fn commands<'a>(path: &'a str, output: &'a str) -> Vec<Vec<&'a str>> {
    vec![
        vec!["inspect", path],
        vec!["tables", path, "--count"],
        vec!["rows", path, "Items"],
        vec!["rows", path, "--all"],
        vec!["rows", path, "Items", "--stream"],
        vec!["query", "run", "SELECT Id FROM Items", "--database", path],
        vec!["query", "saved", "Missing", "--database", path],
        vec!["encryption", "inspect", path],
        vec![
            "encryption",
            "add",
            path,
            "--output",
            output,
            "--new-password",
            "public test",
        ],
        vec!["encryption", "remove", path, "--output", output],
        vec![
            "encryption",
            "change-password",
            path,
            "--output",
            output,
            "--new-password",
            "public test",
        ],
    ]
}

#[test]
fn cooperating_exclusive_writers_block_native_reads_before_output_or_copy_creation() {
    let directory = tempfile::tempdir().unwrap();
    for (code, extension) in [(1, "mdb"), (2, "accdb"), (5, "accdb")] {
        let mut bytes = fixture::database();
        bytes[20] = code;
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let path = directory.path().join(format!("input-{code}.{extension}"));
        let output = directory.path().join(format!("output-{code}.{extension}"));
        fs::write(&path, &bytes).unwrap();
        let writer = ExclusiveFile::open(&path).unwrap();

        for command in commands(path.to_str().unwrap(), output.to_str().unwrap()) {
            let result = run(&command);
            assert_eq!(result.status.code(), Some(2), "{command:?}");
            assert!(result.stdout.is_empty(), "{command:?}");
            assert!(
                String::from_utf8_lossy(&result.stderr).contains("lock is busy"),
                "{command:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(!output.exists());
        }
        writer.close().unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);

        let result = run(&[
            "query",
            "run",
            "SELECT Id FROM Items ORDER BY Id",
            "--database",
            path.to_str().unwrap(),
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"Id\n1\n2\n3\n");
        ExclusiveFile::open(&path).unwrap().close().unwrap();
    }
}

#[test]
fn shared_readers_coexist_and_failed_reads_release_cli_locks() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.mdb");
    let bytes = fixture::database();
    fs::write(&path, &bytes).unwrap();
    let reader = SharedFile::open(&path).unwrap();
    let result = run(&[
        "query",
        "run",
        "SELECT Id FROM Items ORDER BY Id",
        "--database",
        path.to_str().unwrap(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Id\n1\n2\n3\n");
    reader.close().unwrap();
    ExclusiveFile::open(&path).unwrap().close().unwrap();

    for command in [
        vec!["rows", path.to_str().unwrap(), "Missing"],
        vec![
            "query",
            "run",
            "SELECT 1/0 FROM Items",
            "--database",
            path.to_str().unwrap(),
        ],
    ] {
        let result = run(&command);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        ExclusiveFile::open(&path).unwrap().close().unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn encrypted_sources_refuse_busy_before_credentials_and_release_after_password_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("encrypted.accdb");
    let plain = fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let bytes = crypto::encrypted_image(&plain, 5, "public test password");
    fs::write(&path, &bytes).unwrap();
    let writer = ExclusiveFile::open(&path).unwrap();
    let command = [
        "query",
        "run",
        "SELECT Count(*) AS Total FROM ROULETTE_TB",
        "--database",
        path.to_str().unwrap(),
    ];
    let result = run(&command);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("lock is busy"));
    writer.close().unwrap();

    for password in [None, Some("wrong public password")] {
        let mut arguments = command.to_vec();
        if let Some(password) = password {
            arguments.extend(["-p", password]);
        }
        let result = run(&arguments);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        ExclusiveFile::open(&path).unwrap().close().unwrap();
    }

    let reader = SharedFile::open(&path).unwrap();
    let mut arguments = command.to_vec();
    arguments.extend(["-p", "public test password"]);
    let result = run(&arguments);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Total\n38\n");
    reader.close().unwrap();
    ExclusiveFile::open(&path).unwrap().close().unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
