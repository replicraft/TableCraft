//! Private source-only export draft; no observer or producer output is an oracle.
use super::super::indexed_sources::export::expected;
use super::model::{self, Case, Recipe, Refusal};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed::{self as originals, Source};
use tablecraft_testkit::support::ace::artifacts::finish_export_directory;

const PAGE: usize = 4096;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn create(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn observation(recipe: Recipe, case: &Case, after: bool) -> Source {
    // This call consumes qualified ORIGINAL metadata only. It never asks an
    // earlier owner to construct an AFTER or invokes native format decoders.
    let original = originals::source(case.original_recipe);
    let bytes = if after {
        case.expected.as_ref().unwrap().clone()
    } else {
        case.before.clone()
    };
    let rows = if after {
        case.final_rows.clone()
    } else {
        case.rows.clone()
    };
    let manual_tree = matches!(recipe, Recipe::ManualTree | Recipe::PrimaryNull);
    let last_counter = if manual_tree {
        assert_eq!(bytes[7 * PAGE + 75 + 15], 3);
        assert_eq!(&bytes[7 * PAGE + 20..7 * PAGE + 24], &[0; 4]);
        None
    } else {
        original.last_counter
    };
    if let Some(last) = last_counter {
        assert_eq!(bytes[7 * PAGE + 75 + 15], 7);
        assert_eq!(&bytes[7 * PAGE + 20..7 * PAGE + 24], &last.to_le_bytes());
    }
    assert_eq!(bytes.len(), original.bytes.len());
    Source {
        bytes,
        table: original.table,
        columns: original.columns,
        rows,
        target: original.target,
        target_position: original.target_position,
        last_counter,
        index_history: original.index_history,
        indexes: original.indexes,
        root: original.root,
        index_pages: original.index_pages,
        allocated: original.allocated,
        free: original.free,
        // This field directs a SOURCE formatter only. Refusal requests still
        // have healthy BEFORE read views and never acquire write authority.
        ordinary: true,
    }
}

fn token(value: Option<i32>) -> String {
    value.map_or_else(|| "N".to_owned(), |value| format!("I32:{value}"))
}

fn refusal(value: Option<Refusal>) -> &'static str {
    match value {
        None => "candidate",
        Some(Refusal::DuplicateKey) => "DuplicateKey",
        Some(Refusal::Conflict) => "Conflict",
        Some(Refusal::GeneratedColumn) => "GeneratedColumn",
        Some(Refusal::RequiredIndexKey) => "RequiredIndexKey",
    }
}

#[test]
#[ignore = "explicit fresh absolute source directory; no automatic export"]
fn export_ordinary_indexed_target_originals_and_literal_after_views() {
    let directory = env::var_os("TABLECRAFT_ORDINARY_INDEXED_TARGET_SOURCE_DIR")
        .expect("explicit fresh source directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute() && !directory.exists());
    fs::create_dir(directory).unwrap();
    let mut inventory = String::from("id\tphase\tbytes\tsha256\n");
    let mut requests = String::from(
        "request\tsource\tcolumn\trow\texpected\treplacement\toutcome\tchanged_rows\n",
    );
    let mut before_count = 0;
    let mut after_count = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let request_id = format!("h{header}-{}", recipe.id());
            let before_id = format!("{request_id}-before");
            if case.updates.is_empty() {
                requests.push_str(&format!(
                    "{request_id}\t{before_id}\tId\t-\t-\t-\t{}\t{}\n",
                    refusal(case.refusal),
                    case.changed_rows
                ));
            }
            for update in &case.updates {
                requests.push_str(&format!(
                    "{request_id}\t{before_id}\tId\t{}\t{}\t{}\t{}\t{}\n",
                    update.row,
                    token(update.expected),
                    token(update.replacement),
                    refusal(case.refusal),
                    case.changed_rows
                ));
            }
            for after in [false, true] {
                if after && case.expected.is_none() {
                    continue;
                }
                let phase = if after { "after" } else { "before" };
                let id = format!("{request_id}-{phase}");
                let original = observation(recipe, &case, after);
                let digest = hex(&Sha256::digest(&original.bytes));
                // Both complete image and exact report are pure source facts,
                // prepared before any independent getter invocation.
                let report = expected(&original, header, &digest);
                create(&directory.join(format!("{id}.accdb")), &original.bytes);
                create(
                    &directory.join(format!("{id}.expected.tsv")),
                    report.as_bytes(),
                );
                inventory.push_str(&format!(
                    "{id}\t{phase}\t{}\t{digest}\n",
                    original.bytes.len()
                ));
                if after {
                    after_count += 1;
                } else {
                    before_count += 1;
                }
            }
        }
    }
    assert_eq!((before_count, after_count), (32, 24));
    create(&directory.join("inventory.tsv"), inventory.as_bytes());
    create(&directory.join("requests.tsv"), requests.as_bytes());
    let _namespace = finish_export_directory(directory).unwrap();
}
