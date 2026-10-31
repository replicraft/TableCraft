//! Explicit create-new export; no implicit source writes or producer calls.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::canonical_nullable_key_before_change::{self as model, Recipe};

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
    let rows = rows
        .iter()
        .map(|row| {
            let Value::Int32(id) = &row[0] else {
                panic!("literal readonly Id");
            };
            let score = match &row[5] {
                Value::Null => "null".to_owned(),
                Value::Int32(value) => value.to_string(),
                _ => panic!("literal Score carrier"),
            };
            format!("[{id},{score}]")
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{rows}]")
}

#[test]
#[ignore = "explicit fresh absolute output; existing source attempts retained"]
fn export_untouched_nullable_key_sources_and_literal_expectations_create_new() {
    let directory =
        env::var_os("TABLECRAFT_NULLABLE_KEY_SOURCE_DIR").expect("explicit output directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    assert!(!directory.exists());
    let mut files = Vec::new();
    let mut requests = Vec::new();
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let id = format!("h{header}-{}", recipe.id());
            let original = format!("{id}-original.accdb");
            let expected = format!("{id}-expected.accdb");
            let components = case
                .components
                .iter()
                .map(|(number, descending)| {
                    format!("{{\"column\":{number},\"descending\":{descending}}}")
                })
                .collect::<Vec<_>>()
                .join(",");
            let owned = case
                .owned
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let nulls = case
                .null_rows
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let inactive = case
                .null_rows
                .iter()
                .map(|ordinal| {
                    let literal = match ordinal {
                        1 => "d0070000",
                        2 => "b80b0000",
                        _ => panic!("literal inactive payload"),
                    };
                    format!("{{\"ordinal\":{ordinal},\"bytes\":\"{literal}\"}}")
                })
                .collect::<Vec<_>>()
                .join(",");
            let hypothesis = recipe == Recipe::UniqueNullPair;
            requests.push(format!("{{\"id\":\"{id}\",\"header\":{header},\"original\":\"{original}\",\"expected\":\"{expected}\",\"native_request\":{{\"table\":\"Items\",\"column\":\"Score\",\"updates\":[{{\"ordinal\":{},\"expected\":{},\"replacement\":{}}}]}},\"expected_outcome\":{{\"kind\":\"candidate\",\"changed_rows\":1}},\"profile\":{{\"unique\":{},\"unique_all_null_read_hypothesis\":{hypothesis},\"node\":{},\"left_count\":{},\"prefix\":0,\"index_name\":\"{}\",\"root\":10,\"usage_map\":785,\"components\":[{components}],\"owned\":[{owned}],\"null_rows\":[{nulls}],\"inactive_scores\":[{inactive}]}},\"original_id_score\":{},\"final_id_score\":{}}}", case.update.ordinal, case.update.expected, case.update.replacement, case.unique, case.node, case.left_count, case.index_name, rows(&case.rows), rows(&case.final_rows)));
            files.push((original, case.before));
            files.push((expected, case.expected));
        }
    }
    assert_eq!((requests.len(), files.len()), (12, 24));
    assert_eq!(
        files.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
        1_146_880
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
