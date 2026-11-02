//! Explicit original/expected export, never a production native writer.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_testkit::fixtures::access::canonical_indexed_before_change as model;

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

#[test]
#[ignore = "explicit root-selected fresh absolute directory; no implicit fixture export"]
fn export_indexed_before_change_sources_and_literal_expectations_create_new() {
    let directory = env::var_os("TABLECRAFT_INDEXED_BEFORE_CHANGE_SOURCE_DIR")
        .expect("explicit output directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    assert!(!directory.exists(), "retain all previous source attempts");
    let mut files = Vec::new();
    let mut requests = Vec::new();
    // Finish the complete immutable image/request inventory before creating any
    // output. Only fixture constructors run; native mutation APIs are absent.
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let id = format!("h{header}-{}", recipe.id());
            let original = format!("{id}-original.accdb");
            let expected = case
                .expected
                .as_ref()
                .map(|_| format!("{id}-expected.accdb"));
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
            let expected_name = expected
                .as_ref()
                .map_or_else(|| "null".to_owned(), |name| format!("\"{name}\""));
            let outcome = if expected.is_some() {
                format!(
                    "{{\"kind\":\"candidate\",\"changed_rows\":{}}}",
                    case.changed_rows
                )
            } else {
                "{\"kind\":\"refusal\",\"reason\":\"unique-final-key-conflict\"}".to_owned()
            };
            requests.push(format!("{{\"id\":\"{id}\",\"header\":{header},\"original\":\"{original}\",\"expected\":{expected_name},\"native_request\":{{\"table\":\"Items\",\"column\":\"Score\",\"updates\":[{updates}]}},\"expected_outcome\":{outcome}}}"));
            files.push((original, case.before));
            if let Some(bytes) = case.expected {
                files.push((expected.unwrap(), bytes));
            }
        }
    }
    assert_eq!((requests.len(), files.len()), (18, 34));
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
