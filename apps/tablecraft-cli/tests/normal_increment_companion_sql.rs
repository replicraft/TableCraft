#![cfg(any(target_os = "macos", target_os = "linux"))]
//! Honest SQL carriers preserve original omission; binary and I64 are typed cases.
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_testkit::{
    fixtures::access::genuine_increment_companions as fixtures, support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires all companion source pins before CLI generation"]
fn sql_omissions_save_twelve_literal_images_and_readbacks_without_advancing_on_refusal() {
    let cases = fixtures::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    });
    let mut saves = 0;

    for case in &cases {
        let Some(sql) = case.sql() else {
            continue;
        };

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        let target = path.to_str().unwrap();
        let journal = bundle.to_str().unwrap();
        fs::write(&path, &case.before).unwrap();

        let result = CLI.run(&[
            "query",
            "journal-insert",
            sql,
            "--database",
            target,
            "--journal",
            journal,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_eq!(result.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());

        let query = format!("SELECT * FROM {}", case.table);
        let rows = CLI.run(&["query", "run", &query, "--database", target]);
        assert!(
            rows.status.success(),
            "{}",
            String::from_utf8_lossy(&rows.stderr)
        );
        assert!(rows.stderr.is_empty());
        assert_eq!(rows.stdout, case.readback().as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());

        // Supplying the generated field explicitly is a different operation,
        // even for SQL NULL. Preserve the saved counter and full prior image.
        let generated = case.columns()[case.generated_position];
        let invalid = sql.replacen('(', &format!("([{generated}], "), 1);
        let invalid = invalid.replacen("VALUES (", "VALUES (NULL, ", 1);
        let failure = CLI.run(&[
            "query",
            "journal-insert",
            &invalid,
            "--database",
            target,
            "--journal",
            journal,
        ]);
        assert_eq!(failure.status.code(), Some(2));
        assert!(failure.stdout.is_empty());
        assert!(
            String::from_utf8(failure.stderr)
                .unwrap()
                .contains("native write profile is unsupported (Column)")
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }

    assert_eq!(saves, 12);
}
