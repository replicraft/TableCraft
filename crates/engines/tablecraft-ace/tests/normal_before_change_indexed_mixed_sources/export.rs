//! Explicit fresh export, independent of any normal planner or writer.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::canonical_indexed_mixed_before_change as model;

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

fn scalar_rows(rows: &[Vec<Value>]) -> String {
    let rows = rows
        .iter()
        .map(|row| {
            let (Value::Int32(id), Value::Int32(score)) = (&row[0], &row[5]) else {
                panic!("literal I32 Id/Score");
            };
            format!("[{id},{score}]")
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{rows}]")
}

#[test]
#[ignore = "explicit fresh absolute directory; every previous export is retained"]
fn export_indexed_mixed_originals_and_literal_expectations_create_new() {
    let directory =
        env::var_os("TABLECRAFT_INDEXED_MIXED_SOURCE_DIR").expect("explicit output directory");
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
            let expected = case
                .expected
                .as_ref()
                .map(|_| format!("{id}-expected.accdb"));
            let expected_name = expected
                .as_ref()
                .map_or_else(|| "null".to_owned(), |name| format!("\"{name}\""));
            let updates = case
                .updates
                .iter()
                .map(|update| {
                    format!(
                        "{{\"ordinal\":{},\"expected\":{},\"replacement\":{}}}",
                        update.ordinal, update.expected, update.replacement
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
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
            let final_rows = case
                .final_rows
                .as_ref()
                .map_or_else(|| "null".to_owned(), |rows| scalar_rows(rows));
            let outcome = if case.expected.is_some() {
                format!(
                    "{{\"kind\":\"candidate\",\"changed_rows\":{}}}",
                    case.changed_rows
                )
            } else {
                // Literal future final key2000 duplicates original row1.
                "{\"kind\":\"refusal\",\"reason\":\"unique-conflict\",\"final_duplicate\":2000}"
                    .to_owned()
            };
            requests.push(format!("{{\"id\":\"{id}\",\"header\":{header},\"original\":\"{original}\",\"expected\":{expected_name},\"native_request\":{{\"table\":\"Items\",\"column\":\"Score\",\"updates\":[{updates}]}},\"expected_outcome\":{outcome},\"profile\":{{\"primary\":{},\"node\":{},\"retained_upper\":{},\"unique\":{},\"index_name\":\"{}\",\"data_number\":{},\"root\":{},\"usage_map\":{},\"components\":[{components}],\"owned\":[{owned}]}},\"original_id_score\":{},\"final_id_score\":{final_rows}}}", case.primary, case.node, case.retained_upper, case.unique, case.index_name, case.data_number, case.root, case.usage_map, scalar_rows(&case.rows)));
            files.push((original, case.before));
            if let Some(after) = case.expected {
                files.push((expected.unwrap(), after));
            }
        }
    }
    assert_eq!((requests.len(), files.len()), (14, 26));
    assert_eq!(
        files.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
        1_220_608
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
