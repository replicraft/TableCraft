//! Explicit create-exclusive SOURCE and literal SQL/request export.
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_before_change::Cell, canonical_fixed_sql_before_change as model,
};

fn create(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn cell(value: Cell) -> String {
    match value {
        Cell::Null => "{\"kind\":\"null\"}".into(),
        Cell::Boolean(value) => format!("{{\"kind\":\"boolean\",\"value\":{value}}}"),
        Cell::Int32(value) => format!("{{\"kind\":\"int32\",\"value\":{value}}}"),
        Cell::Int64(value) => format!("{{\"kind\":\"int64\",\"value\":\"{value}\"}}"),
        Cell::Currency(value) => format!("{{\"kind\":\"currency-scaled\",\"value\":\"{value}\"}}"),
        Cell::Date(bits) => format!("{{\"kind\":\"date-bits\",\"value\":\"{bits:016x}\"}}"),
    }
}

#[test]
#[ignore = "explicit fresh absolute directory; no automatic source export"]
fn export_fixed_sql_discriminator_sources_create_new() {
    let directory =
        env::var_os("TABLECRAFT_FIXED_SQL_SOURCE_DIR").expect("explicit fresh directory");
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
            let groups = case
                .groups
                .iter()
                .map(|group| {
                    let field = model::FIELDS
                        .iter()
                        .find(|field| field.0 == group.column)
                        .unwrap();
                    let update = &group.updates[0];
                    format!(
                        "{{\"column\":\"{}\",\"native_number\":{},\"updates\":[{{\"ordinal\":0,\"expected\":{},\"replacement\":{}}}]}}",
                        field.1,
                        field.0,
                        cell(update.expected),
                        cell(update.replacement)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            requests.push(format!(
                "{{\"id\":\"{id}\",\"header\":{header},\"original\":\"{original}\",\"expected\":\"{expected}\",\"sql\":{:?},\"reversed_sql\":{:?},\"native_request\":{{\"table\":\"Items\",\"groups\":[{groups}]}},\"matched_rows\":1,\"changed_rows\":1}}",
                case.sql, case.reversed_sql,
            ));
            files.push((original, case.before));
            files.push((expected, case.expected));
        }
    }
    assert_eq!((requests.len(), files.len()), (10, 20));
    fs::create_dir(directory).unwrap();
    let mut inventory = Vec::new();
    for (name, bytes) in files {
        create(&directory.join(&name), &bytes);
        let digest = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let entry = format!(
            "{{\"path\":\"{name}\",\"bytes\":{},\"sha256\":\"{digest}\"}}",
            bytes.len()
        );
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
