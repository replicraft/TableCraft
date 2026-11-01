//! Explicit exclusive export of frozen sources and original typed requests.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_testkit::access::canonical_fixed_before_change::{self as model, Cell};

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

fn cell(value: Cell) -> String {
    match value {
        Cell::Null => "{\"kind\":\"null\"}".into(),
        Cell::Boolean(value) => format!("{{\"kind\":\"boolean\",\"value\":{value}}}"),
        Cell::Int32(value) => format!("{{\"kind\":\"int32\",\"value\":{value}}}"),
        Cell::Int64(value) => format!("{{\"kind\":\"int64\",\"value\":\"{value}\"}}"),
        Cell::Currency(value) => {
            format!("{{\"kind\":\"currency-scaled\",\"value\":\"{value}\"}}")
        }
        Cell::Date(bits) => format!("{{\"kind\":\"date-bits\",\"value\":\"{bits:016x}\"}}"),
    }
}

#[test]
#[ignore = "explicit fresh absolute directory; no implicit export or overwrite"]
fn export_fixed_scalar_before_change_sources_create_new() {
    let directory = env::var_os("TABLECRAFT_FIXED_BEFORE_CHANGE_SOURCE_DIR")
        .expect("explicit fresh source directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    assert!(!directory.exists());
    // Construct the complete finite set before creating the output namespace.
    let mut files = Vec::new();
    let mut requests = Vec::new();
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let id = format!("h{header}-{}", recipe.id());
            let before_name = format!("{id}-original.accdb");
            let after_name = case
                .expected
                .as_ref()
                .map(|_| format!("{id}-expected.accdb"));
            let expected_name = after_name
                .as_ref()
                .map_or_else(|| "null".into(), |name| format!("\"{name}\""));
            let groups = case
                .groups
                .iter()
                .map(|group| {
                    let field = model::FIELDS
                        .iter()
                        .find(|field| field.0 == group.column)
                        .unwrap();
                    let updates = group
                        .updates
                        .iter()
                        .map(|update| {
                            format!(
                                "{{\"ordinal\":{},\"expected\":{},\"replacement\":{}}}",
                                update.ordinal,
                                cell(update.expected),
                                cell(update.replacement)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(
                        "{{\"column\":\"{}\",\"native_number\":{},\"updates\":[{updates}]}}",
                        field.1, group.column
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let outcome = match case.refusal {
                None => format!(
                    "{{\"kind\":\"candidate\",\"changed_rows\":{}}}",
                    case.changed_rows
                ),
                Some(refusal) => format!("{{\"kind\":\"refusal\",\"reason\":\"{refusal:?}\"}}"),
            };
            let event_rows = case
                .event_rows
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",");
            requests.push(format!(
                "{{\"id\":\"{id}\",\"header\":{header},\"original\":\"{before_name}\",\"expected\":{expected_name},\"native_request\":{{\"table\":\"Items\",\"groups\":[{groups}]}},\"event_rows\":[{event_rows}],\"expected_outcome\":{outcome}}}"
            ));
            files.push((before_name, case.before));
            if let Some(expected) = case.expected {
                files.push((after_name.unwrap(), expected));
            }
        }
    }
    assert_eq!((requests.len(), files.len()), (38, 66));
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
    fs::File::open(directory).unwrap().sync_all().unwrap();
}
