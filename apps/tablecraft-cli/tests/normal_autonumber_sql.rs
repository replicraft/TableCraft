#![cfg(any(target_os = "macos", target_os = "linux"))]
//! Existing journal-insert SQL uses source-qualified omission and retains errors.
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_testkit::fixtures::access::genuine_normal_autonumber as fixtures;

fn insert(path: &Path, bundle: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "journal-insert",
            "INSERT INTO Items ([Score]) VALUES (17)",
        ])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(bundle)
        .output()
        .unwrap()
}

#[test]
#[ignore = "requires14 independent source native/SDK views before matching normal generation"]
fn cli_omission_saves_four_whole_images_and_reports_exhaustion_without_effects() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in fixtures::RECIPES {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, sources.bytes(header, recipe.before())).unwrap();
            let output = insert(&path, &bundle);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
            assert_eq!(
                fs::read(&path).unwrap(),
                sources.bytes(header, recipe.after())
            );
            assert!(!bundle.exists());
            let rows = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "run", "SELECT [Id], [Score] FROM Items"])
                .arg("--database")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                rows.status.success(),
                "{}",
                String::from_utf8_lossy(&rows.stderr)
            );
            assert!(rows.stderr.is_empty());
            let expected = match recipe {
                fixtures::Recipe::High => "Id\tScore\n1\t10\n138\t17\n",
                fixtures::Recipe::Zero => "Id\tScore\n1\t17\n",
            };
            assert_eq!(rows.stdout, expected.as_bytes());
            assert_eq!(
                fs::read(&path).unwrap(),
                sources.bytes(header, recipe.after())
            );
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("exhausted.accdb");
        let bundle = directory.path().join("journal");
        let before = sources.bytes(header, "increment-maximum");
        fs::write(&path, before).unwrap();
        let output = insert(&path, &bundle);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("incrementing AutoNumber range is exhausted")
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    assert_eq!(saves, 4);
}
