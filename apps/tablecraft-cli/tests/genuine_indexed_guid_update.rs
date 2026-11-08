#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_core::values::Value;
use tablecraft_testkit::{
    fixtures::access::genuine_indexed_guid_update::requests::{self as source, Case},
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

fn save(case: &Case, score_first: Option<bool>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("target.accdb");
    let journal = directory.path().join("journal");
    fs::write(&path, &case.before).unwrap();
    let sql = if let Some(reverse) = score_first {
        let guid = "[Group]={GUID {11223344-5566-7788-99aa-bbccddeeff00}}";
        let assignments = if reverse {
            format!("[Score]=-63,{guid}")
        } else {
            format!("{guid},[Score]=-63")
        };
        format!("UPDATE Items SET {assignments} WHERE [ID]=1")
    } else {
        case.sql()
    };
    let output = CLI.run(&[
        "query",
        "journal-update",
        &sql,
        "--database",
        path.to_str().unwrap(),
        "--journal",
        journal.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let count = case.changed_rows();
    assert_eq!(
        output.stdout,
        format!(
            "Transaction\tCommitted\n\
        Matched rows\t{count}\n\
        Changed rows\t{count}\n\
        Journal namespace\tRetired\n\
        Retirement guarantee\tCaptured-parent barrier\n\
        Journal cleanup\tRemoved\n"
        )
        .as_bytes()
    );
    let expected = if score_first.is_some() {
        case.composite_after()
    } else {
        case.after.to_vec()
    };
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert!(!journal.exists());

    let mut ordinals: Vec<_> = case.updates().iter().map(|update| update.0).collect();
    ordinals.sort_unstable();
    let ids = ordinals
        .iter()
        .map(|ordinal| (ordinal + 1).to_string())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT [ID],[Group],[Score] FROM Items WHERE [ID] IN({ids}) ORDER BY [ID]");
    let output = CLI.run(&["query", "run", &sql, "--database", path.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let mut rows = case.rows(true);
    if score_first.is_some() {
        rows[0][2] = Value::Int32(-63);
    }
    let mut text = "ID\tGroup\tScore\n".to_owned();
    for ordinal in ordinals {
        let Value::Int32(score) = rows[ordinal][2] else {
            unreachable!()
        };
        text.push_str(&format!(
            "{}\t{}\t{score}\n",
            ordinal + 1,
            source::display(&rows[ordinal][1])
        ));
    }
    assert_eq!(output.stdout, text.as_bytes());
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
#[ignore = "24 distinct SQL requests and eight written-order GUID/Score atomic batches"]
fn cli_indexed_guid_literals_save_exact_images_and_canonical_readbacks() {
    let cases = source::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut counts = [0; 2];
    for case in cases.into_iter().filter(|case| !case.reverse) {
        save(&case, None);
        counts[0] += 1;
        if case.kind == 1 {
            for reverse in [false, true] {
                save(&case, Some(reverse));
                counts[1] += 1;
            }
        }
    }
    assert_eq!(counts, [24, 8]);
}
