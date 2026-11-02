#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_testkit::fixtures::access::genuine_default_insert::{
    self as sources, RECIPES, Recipe,
};

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
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn cli_partial_and_explicit_insert_fields_match_literal_images_and_observed_cells() {
    let mut saves = 0;
    for case in sources::load_all(verify) {
        for recipe in RECIPES {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let output = insert(&path, &bundle, recipe.sql());
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
            assert_eq!(fs::read(&path).unwrap(), case.expected(recipe));
            assert!(!bundle.exists());
            let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args([
                    "query",
                    "run",
                    "SELECT [Id], [Score], [Caption], [Optional] FROM Defaults",
                ])
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
            let rows = match recipe {
                Recipe::OmittedConstants => {
                    "Id\tScore\tCaption\tOptional\n1\t-1\t\"\"\"original\"\"\"\tNULL\n24\t7\t\"\"\"seed\"\"\"\tNULL\n"
                }
                Recipe::ExplicitNull => {
                    "Id\tScore\tCaption\tOptional\n1\t-1\t\"\"\"original\"\"\"\tNULL\n25\tNULL\tNULL\tNULL\n"
                }
                Recipe::AllOverrides => {
                    "Id\tScore\tCaption\tOptional\n1\t-1\t\"\"\"original\"\"\"\tNULL\n26\t-9\t\"\"\"override\"\"\"\t99\n"
                }
            };
            assert_eq!(read.stdout, rows.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.expected(recipe));
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 6);
}
