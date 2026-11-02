//! Explicit create-exclusive export of the frozen pure SOURCE corpus only.
use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tablecraft_testkit::fixtures::access::canonical_before_change::{self as model, Recipe};

const HEX_DIGITS_PER_BYTE: usize = 2;

pub(super) fn digest_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * HEX_DIGITS_PER_BYTE);
    for byte in digest.iter() {
        write!(&mut text, "{byte:02x}").unwrap();
    }
    text
}

pub(super) fn identity(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::Sequential => "sequential",
        Recipe::MacroOnlyTarget => "macro-only-target",
        Recipe::UserNoopMacroChanges => "user-noop-macro-changes",
        Recipe::RestoreOriginal => "restore-original",
        Recipe::Empty => "empty",
        Recipe::AllNoop => "all-noop",
        Recipe::DormantDelete => "dormant-delete",
        Recipe::LateRaise => "late-raise",
        Recipe::AfterUpdate => "after-update",
        Recipe::Named => "named",
        Recipe::NamesOnly => "names-only",
        Recipe::Unrepresented => "unrepresented",
        Recipe::OpaqueAction => "opaque-action",
        Recipe::BodyPresent => "body-present",
        Recipe::Stale => "stale",
        Recipe::Duplicate => "duplicate",
    }
}

fn normal_scope(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::UserNoopMacroChanges | Recipe::AllNoop => "supplied-equal-assignment-refusal",
        Recipe::Sequential
        | Recipe::MacroOnlyTarget
        | Recipe::RestoreOriginal
        | Recipe::Empty
        | Recipe::DormantDelete => "requires-independent-normal-admission",
        _ => "modeled-refusal-no-candidate",
    }
}

fn create(path: &Path) -> fs::File {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
}

fn image(directory: &Path, filename: &str, bytes: &[u8]) -> String {
    assert_eq!(bytes.len(), 36_864);
    let path = directory.join(filename);
    let mut output = create(&path);
    output.write_all(bytes).unwrap();
    output.flush().unwrap();
    drop(output);
    let actual = fs::read(path).unwrap();
    assert_eq!(actual, bytes);
    digest_hex(&actual)
}

#[test]
#[ignore = "explicit fresh pure SOURCE export; no normal planner or macro evaluator"]
fn export_normal_before_change_originals_and_modeled_outcomes_create_exclusive() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_NORMAL_BEFORE_CHANGE_SOURCE_DIR")
            .expect("an explicit absolute fresh export directory is required"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut inventory = create(&directory.join("inventory.tsv"));
    let header = concat!(
        "header\trecipe\trole\tfilename\tbytes\tsha256\t",
        "supplied_updates\tmodeled_changed_rows\tnormal_scope\n",
    );
    inventory.write_all(header.as_bytes()).unwrap();
    let mut requests = create(&directory.join("requests.tsv"));
    requests
        .write_all(b"header\trecipe\tcolumn\tordinal\texpected\treplacement\n")
        .unwrap();
    let mut originals = 0;
    let mut outcomes = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            assert_eq!(case.header, header);
            assert_eq!(case.recipe, recipe);
            let id = identity(recipe);
            let filename = format!("normal-before-change-h{header}-{id}-before.accdb");
            let hash = image(&directory, &filename, &case.before);
            writeln!(
                inventory,
                "{header}\t{id}\toriginal\t{filename}\t36864\t{hash}\t{}\t{}\t{}",
                case.updates.len(),
                case.changed_rows,
                normal_scope(recipe)
            )
            .unwrap();
            originals += 1;
            if let Some(expected) = &case.expected {
                let filename = format!("normal-before-change-h{header}-{id}-expected.accdb");
                let hash = image(&directory, &filename, expected);
                writeln!(
                    inventory,
                    "{header}\t{id}\tmodeled-after-image\t{filename}\t36864\t{hash}\t{}\t{}\t{}",
                    case.updates.len(),
                    case.changed_rows,
                    normal_scope(recipe)
                )
                .unwrap();
                outcomes += 1;
            }
            for update in &case.updates {
                writeln!(
                    requests,
                    "{header}\t{id}\tScore\t{}\t{}\t{}",
                    update.ordinal, update.expected, update.replacement
                )
                .unwrap();
            }
        }
    }
    assert_eq!((originals, outcomes), (32, 14));
    inventory.flush().unwrap();
    requests.flush().unwrap();
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 48);
}
