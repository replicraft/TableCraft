#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_testkit::{
    fixtures::access::genuine_wide_root_promotion as sources, support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires literal wider root promotion sources before normal CLI saves"]
fn cli_explicit_scalar_promotions_publish_exact_whole_images_and_retained_scalar_readbacks() {
    let cases = sources::load_all(|bytes, expected| {
        let actual: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    });
    let mut commits = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = CLI.run(&[
            "query",
            "journal-insert",
            &case.sql(),
            "--database",
            path.to_str().unwrap(),
            "--journal",
            bundle.to_str().unwrap(),
        ]);
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
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        let query = format!(
            "SELECT [ID] AS NewID, [Marker] AS NewMarker FROM [{}] WHERE [ID] = 401",
            case.table
        );
        let read = CLI.run(&["query", "run", &query, "--database", path.to_str().unwrap()]);
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        assert_eq!(read.stdout, b"NewID\tNewMarker\n401\t6817\n");
        let query = format!("SELECT Count(*) AS Total FROM [{}]", case.table);
        let read = CLI.run(&["query", "run", &query, "--database", path.to_str().unwrap()]);
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        assert_eq!(read.stdout, b"Total\n401\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        commits += 1;
    }
    assert_eq!(commits, 4);
}

#[test]
#[ignore = "requires qualified prefix3 originals; SQL append-query grammar remains refused"]
fn cli_multirow_append_query_refuses_before_journal_or_target_effects() {
    let cases = tablecraft_testkit::fixtures::access::genuine_wide_primary_prefix_growth::load_all(
        |bytes, expected| {
            let actual: String = Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(actual, expected);
        },
    );
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = CLI.run(&[
            "query",
            "journal-insert",
            &case.unsupported_batch_sql(),
            "--database",
            path.to_str().unwrap(),
            "--journal",
            bundle.to_str().unwrap(),
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("query feature is not implemented")
        );
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
