#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_core::values::Value;
use tablecraft_testkit::{
    fixtures::access::genuine_external_lval_mixed as fixture, support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn cli_external_mixed_null_and_text_saves_have_exact_files_receipts_and_readback() {
    let cases = fixture::load_all(|raw, expected| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    });
    let mut saves = 0;
    for case in cases.iter().filter(|case| case.recipe != "original") {
        for reverse in [false, true] {
            let Some(statement) = fixture::statement(case, reverse) else {
                continue;
            };
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let output = CLI.run(&[
                "query",
                "journal-update",
                &statement,
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
            let changed = fixture::changed_rows(case);
            assert_eq!(output.stdout, format!("Transaction\tCommitted\nMatched rows\t{changed}\nChanged rows\t{changed}\nJournal namespace\tRetired and synchronized\n").as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            let output = CLI.run(&["query", "run", "SELECT [ID], IsNull([Memo]) AS MemoMissing, Len([Memo]) AS MemoLength, IsNull([Blob]) AS BlobMissing FROM Items", "--database", path.to_str().unwrap()]);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let mut expected = String::from("ID\tMemoMissing\tMemoLength\tBlobMissing\n");
            for row in &case.rows {
                let Value::Int32(id) = row[0] else {
                    unreachable!()
                };
                let memo_null = matches!(row[3], Value::Null);
                let blob_null = matches!(row[2], Value::Null);
                let length = match &row[3] {
                    Value::Null => "NULL".to_owned(),
                    Value::Text(value) => value.encode_utf16().count().to_string(),
                    _ => unreachable!(),
                };
                expected.push_str(&format!(
                    "{id}\t{}\t{length}\t{}\n",
                    if memo_null { "TRUE" } else { "FALSE" },
                    if blob_null { "TRUE" } else { "FALSE" }
                ));
            }
            assert_eq!(output.stdout, expected.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 16);
}
