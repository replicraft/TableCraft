#![cfg(any(target_os = "macos", target_os = "linux"))]
//! No batch SQL grammar is introduced: single-row supplied-generator refusal is inert.
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_testkit::{
    fixtures::access::genuine_increment_multi as fixtures, support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires all four multi-index source pins; positive single SQL lives in companion regression"]
fn explicit_generated_null_does_not_consume_a_counter_or_create_a_journal() {
    let cases = fixtures::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let result = CLI.run(&[
            "query",
            "journal-insert",
            "INSERT INTO Items (Id, Score, Guard, Label) VALUES (NULL, CLng(17), CLng(7), 'new')",
            "--database",
            path.to_str().unwrap(),
            "--journal",
            bundle.to_str().unwrap(),
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("native write profile is unsupported (Column)")
        );
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
