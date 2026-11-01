//! Explicit companion export, separate from the frozen twelve Score cases.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::canonical_nullable_index_id_before_change as model;

fn create(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn pin(name: &str, bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "{{\"path\":\"{name}\",\"bytes\":{},\"sha256\":\"{hash}\"}}",
        bytes.len()
    )
}

fn rows(rows: &[Vec<Value>]) -> String {
    fn value(value: &Value) -> String {
        match value {
            Value::Null => "null".to_owned(),
            Value::Int32(value) => value.to_string(),
            _ => panic!("literal nullable I32"),
        }
    }
    let rows = rows
        .iter()
        .map(|row| format!("[{},{}]", value(&row[0]), value(&row[5])))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{rows}]")
}

#[test]
#[ignore = "explicit fresh absolute companion directory; no overwrite"]
fn export_nullable_present_id_companion_create_new() {
    let directory =
        env::var_os("TABLECRAFT_NULLABLE_ID_SOURCE_DIR").expect("explicit output directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    assert!(!directory.exists());
    let mut files = Vec::new();
    let mut requests = Vec::new();
    for header in [5, 6] {
        let case = model::source(header);
        let original = format!("h{header}-present-id-original.accdb");
        let expected = format!("h{header}-present-id-expected.accdb");
        requests.push(format!("{{\"id\":\"h{header}-present-id\",\"header\":{header},\"original\":\"{original}\",\"expected\":\"{expected}\",\"native_request\":{{\"table\":\"Items\",\"column\":\"Id\",\"updates\":[{{\"ordinal\":0,\"expected\":1,\"replacement\":7}}]}},\"expected_outcome\":{{\"kind\":\"candidate\",\"changed_rows\":1}},\"profile\":{{\"index_name\":\"PrimaryId\",\"ordinary_index\":true,\"generated_id\":false,\"root\":9,\"usage_map\":784,\"prefix\":0,\"null_ordinal\":2,\"inactive_id\":\"03000000\",\"macro_only_score\":2007}},\"original_id_score\":{},\"final_id_score\":{}}}", rows(&case.rows), rows(&case.final_rows)));
        files.push((original, case.before));
        files.push((expected, case.expected));
    }
    assert_eq!((requests.len(), files.len()), (2, 4));
    assert_eq!(
        files.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
        163_840
    );
    fs::create_dir(directory).unwrap();
    let mut inventory = Vec::new();
    for (name, bytes) in files {
        create(&directory.join(&name), &bytes);
        let entry = pin(&name, &bytes);
        println!("{entry}");
        inventory.push(entry);
    }
    create(
        &directory.join("inventory.json"),
        format!("{{\"files\":[{}]}}\n", inventory.join(",")).as_bytes(),
    );
    create(
        &directory.join("requests.json"),
        format!("{{\"cases\":[{}]}}\n", requests.join(",")).as_bytes(),
    );
}
