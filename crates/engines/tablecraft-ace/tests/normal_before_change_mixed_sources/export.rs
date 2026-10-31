//! Explicit A/B source export; rich source export has a separate namespace/gate.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::canonical_mixed_before_change as model;

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
#[ignore = "explicit fresh absolute A/B directory; no implicit export or overwrite"]
fn export_mixed_scalar_originals_and_literal_expectations_create_new() {
    let directory =
        env::var_os("TABLECRAFT_MIXED_SCALAR_SOURCE_DIR").expect("explicit output directory");
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
            let outcome = match case.refusal {
                None => format!(
                    "{{\"kind\":\"candidate\",\"changed_rows\":{}}}",
                    case.changed_rows
                ),
                Some(model::Refusal::GeneratedTarget) => {
                    "{\"kind\":\"refusal\",\"reason\":\"generated-target\"}".into()
                }
                Some(model::Refusal::Operand) => {
                    "{\"kind\":\"refusal\",\"reason\":\"non-I32-operand\"}".into()
                }
                Some(model::Refusal::Raised(number)) => {
                    format!("{{\"kind\":\"refusal\",\"reason\":\"raised\",\"number\":{number}}}")
                }
            };
            let final_targets = case.final_rows.as_ref().map_or_else(
                || "null".to_owned(),
                |rows| {
                    let values = rows
                        .iter()
                        .map(|row| {
                            let Value::Int32(value) = &row[case.target] else {
                                panic!("literal target I32");
                            };
                            value.to_string()
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("[{values}]")
                },
            );
            requests.push(format!(
                concat!(
                    "{{\"id\":\"{id}\"",
                    ",\"header\":{header}",
                    ",\"original\":\"{original}\"",
                    ",\"expected\":{expected_name}",
                    ",\"native_request\":{{\"table\":\"Items\"",
                    ",\"column\":\"{}\"",
                    ",\"updates\":[{updates}]}}",
                    ",\"profile\":{{\"target\":{}",
                    ",\"fixed_offset\":{}",
                    ",\"counter\":{}",
                    ",\"primary\":{}}}",
                    ",\"expected_outcome\":{outcome}",
                    ",\"final_targets\":{final_targets}}}",
                ),
                case.column,
                case.target,
                case.offset,
                case.counter,
                case.primary,
                id = id,
                header = header,
                original = original,
                expected_name = expected_name,
                updates = updates,
                outcome = outcome,
                final_targets = final_targets,
            ));
            files.push((original, case.before));
            if let Some(after) = case.expected {
                files.push((expected.unwrap(), after));
            }
        }
    }
    assert_eq!((requests.len(), files.len()), (24, 40));
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
