//! Explicit fresh export of nullable Int32 sources, without writer execution.
use super::model::{self, Recipe};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_testkit::support::ace::artifacts::finish_export_directory;

fn create(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn number(value: Option<i32>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

#[test]
#[ignore = "explicit fresh absolute directory; never exports automatically"]
fn export_nullable_fixed_int32_sources_create_new() {
    let directory = env::var_os("TABLECRAFT_NULLABLE_FIXED_INT32_SOURCE_DIR")
        .expect("explicit fresh directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    assert!(!directory.exists());
    let mut files = Vec::new();
    let mut requests = Vec::new();
    for header in [5, 6] {
        for recipe in [Recipe::Activate, Recipe::Clear] {
            let case = model::source(header, recipe);
            let id = format!("h{header}-{}", recipe.id());
            let before = format!("{id}-original.accdb");
            let expected = format!("{id}-expected.accdb");
            requests.push(format!(
                "{{\"id\":\"{id}\",\"header\":{header},\"original\":\"{before}\",\"expected\":\"{expected}\",\"table\":\"Items\",\"column\":\"Id\",\"ordinal\":0,\"original_value\":{},\"replacement_value\":{},\"macro_only_score\":{},\"changed_rows\":1}}",
                number(case.expected_id), number(case.replacement_id),
                if recipe == Recipe::Activate { "2007" } else { "null" }
            ));
            files.push((before, case.before));
            files.push((expected, case.expected));
        }
    }
    fs::create_dir(directory).unwrap();
    let mut inventory = Vec::new();
    for (name, bytes) in files {
        create(&directory.join(&name), &bytes);
        let hash = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let item = format!(
            "{{\"path\":\"{name}\",\"bytes\":{},\"sha256\":\"{hash}\"}}",
            bytes.len()
        );
        println!("{item}");
        inventory.push(item);
    }
    create(
        &directory.join("inventory.json"),
        format!("{{\"files\":[{}]}}\n", inventory.join(",")).as_bytes(),
    );
    create(
        &directory.join("requests.json"),
        format!("{{\"cases\":[{}]}}\n", requests.join(",")).as_bytes(),
    );
    let _namespace = finish_export_directory(directory).unwrap();
}
