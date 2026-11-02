//! Explicit create-new export and first hash dump, with literal request inventory.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_testkit::fixtures::access::canonical_tree_before_change as model;

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

fn rows(rows: &[[i32; 2]; 3]) -> String {
    format!(
        "[{}]",
        rows.iter()
            .map(|row| format!("[{},{}]", row[0], row[1]))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[test]
#[ignore = "explicit fresh absolute directory; no implicit export or overwrite"]
fn export_retained_tree_sources_and_fitting_expectations_create_new() {
    let directory = env::var_os("TABLECRAFT_TREE_BEFORE_CHANGE_SOURCE_DIR")
        .expect("explicit fresh output directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    assert!(!directory.exists(), "retain every previous source attempt");
    let mut files = Vec::new();
    let mut requests = Vec::new();
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let id = format!("h{header}-{}", recipe.id());
            let original = format!("{id}-original.accdb");
            let expected = format!("{id}-expected.accdb");
            let updates = case
                .updates
                .iter()
                .map(|update| {
                    format!(
                        "{{\"ordinal\":{},\"expected\":{},\"replacement\":{}}}",
                        update.ordinal, update.expected, update.replacement,
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let components = case
                .components
                .iter()
                .map(|(column, descending)| {
                    format!("{{\"column\":{column},\"descending\":{descending}}}")
                })
                .collect::<Vec<_>>()
                .join(",");
            let owned = case
                .owned
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            requests.push(format!(
                "{{\"id\":\"{id}\",\"header\":{header},\"original\":\"{original}\",\"expected\":\"{expected}\",\"native_request\":{{\"table\":\"Items\",\"column\":\"Score\",\"updates\":[{updates}]}},\"expected_outcome\":{{\"kind\":\"candidate\",\"changed_rows\":{}}},\"profile\":{{\"node\":{},\"zero_tail\":{},\"retained_upper\":{},\"unique\":{},\"components\":[{components}],\"owned\":[{owned}]}},\"final_rows\":{}}}",
                case.changed_rows, case.node, case.zero_tail, case.retained_upper, case.unique, rows(&case.final_rows),
            ));
            files.push((original, case.before));
            files.push((expected, case.expected));
        }
    }
    assert_eq!((requests.len(), files.len()), (18, 36));
    assert_eq!(
        files.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
        1_703_936
    );
    fs::create_dir(directory).unwrap();
    let mut inventory = Vec::new();
    for (name, bytes) in files {
        create(&directory.join(&name), &bytes);
        let value = pin(&name, &bytes);
        println!("{value}");
        inventory.push(value);
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
