#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::access::genuine_wide_node_update::{self as source, Recipe};

#[test]
#[ignore = "requires qualified retained wider NODE UPDATE sources"]
fn cli_retained_numeric_original_noop_preserves_complete_image_and_readback() {
    let cases = source::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    let mut saves = 0;
    for case in cases.iter().filter(|case| case.recipe == Recipe::Noop) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        // An original typed field reference preserves its actual carrier; this
        // does not invent SQL coercion of Int32 literals into I64 or Currency.
        let sql = format!(
            "UPDATE [{}] SET [KeyValue] = [KeyValue] WHERE [ID] = 1",
            case.table
        );
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-update", &sql])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&journal)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t0\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());

        let sql = format!("SELECT [ID], [Marker] FROM [{}] WHERE [ID] = 1", case.table);
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", &sql])
            .arg("--database")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"ID\tMarker\n1\t17\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 4);
}
