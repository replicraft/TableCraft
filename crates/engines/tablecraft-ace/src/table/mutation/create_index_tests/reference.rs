//! Explicit finite export for independent public-library comparisons.
use super::{model, qualify};
use std::{fmt::Write, fs, path::Path};

const DIRECTORY: &str = ".cache/ace16-secondary-index-public-reference-20261030/export";

#[test]
#[ignore = "writes a fresh private comparison corpus; run explicitly from the repository root"]
fn export_frozen_secondary_index_sources_and_expectations() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(DIRECTORY);
    fs::create_dir(&directory).unwrap();
    let mut requests = String::new();

    for code in [5, 6] {
        for (population, label) in [
            (model::Population::Empty, "empty"),
            (model::Population::Distinct, "distinct"),
            (model::Population::NullableDuplicate, "nullable"),
            (model::Population::Pressure, "pressure"),
        ] {
            let source_id = format!("h{code}-{label}");
            for descending in [false, true] {
                let case = model::source(code, population, descending);
                qualify(&case.before, &case, false);
                qualify(&case.expected, &case, true);

                let source = directory.join(format!("{source_id}.accdb"));
                if descending {
                    assert_eq!(fs::read(&source).unwrap(), case.before);
                } else {
                    fs::write(&source, &case.before).unwrap();
                    let mut rows = String::new();
                    for row in &case.rows {
                        write_row(&mut rows, row);
                    }
                    fs::write(directory.join(format!("{source_id}-rows.tsv")), rows).unwrap();
                }

                let direction = if descending { "desc" } else { "asc" };
                let case_id = format!("{source_id}-{direction}");
                fs::write(
                    directory.join(format!("{case_id}-expected.accdb")),
                    &case.expected,
                )
                .unwrap();

                let mut order = String::new();
                for entry in &case.new_stream {
                    let reference =
                        u32::from_be_bytes(entry[entry.len() - 4..].try_into().unwrap());
                    let row = case
                        .rows
                        .iter()
                        .find(|row| row.reference == reference)
                        .unwrap();
                    write_row(&mut order, row);
                }
                fs::write(directory.join(format!("{case_id}-order.tsv")), order).unwrap();
                writeln!(
                    requests,
                    "{case_id}\t{source_id}\t{}\t{direction}\t{}\t{}",
                    case.name,
                    case.rows.len(),
                    case.new_counter,
                )
                .unwrap();
            }
        }
    }

    assert_eq!(requests.lines().count(), 16);
    fs::write(directory.join("requests.tsv"), requests).unwrap();
}

fn write_row(output: &mut String, row: &model::Row) {
    let score = row
        .score
        .map_or_else(|| "null".to_owned(), |value| value.to_string());
    writeln!(output, "ROW\t{}\t{score}\t{}", row.id, row.guard).unwrap();
}

/// Verify the actual moved model against the root's preserved pre-move export.
/// No writer, adapter, CLI or reference-library call occurs in this gate.
#[test]
#[ignore = "requires the immutable root-exported pre-extraction corpus"]
fn qualify_extracted_secondary_index_builder_complete_byte_identity() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(DIRECTORY);
    let mut requests = String::new();
    let mut checked_sources = 0;
    let mut checked_expected = 0;

    for code in [5, 6] {
        for (population, label) in [
            (model::Population::Empty, "empty"),
            (model::Population::Distinct, "distinct"),
            (model::Population::NullableDuplicate, "nullable"),
            (model::Population::Pressure, "pressure"),
        ] {
            let source_id = format!("h{code}-{label}");
            for descending in [false, true] {
                let case = model::source(code, population, descending);
                let original = fs::read(directory.join(format!("{source_id}.accdb"))).unwrap();
                assert!(
                    original == case.before,
                    "source={source_id}, descending={descending}, first mismatch={:?}",
                    original
                        .iter()
                        .zip(&case.before)
                        .position(|(left, right)| left != right),
                );
                if !descending {
                    checked_sources += 1;
                    let mut rows = String::new();
                    for row in &case.rows {
                        write_row(&mut rows, row);
                    }
                    assert_eq!(
                        fs::read(directory.join(format!("{source_id}-rows.tsv"))).unwrap(),
                        rows.as_bytes(),
                    );
                }

                let direction = if descending { "desc" } else { "asc" };
                let case_id = format!("{source_id}-{direction}");
                let expected =
                    fs::read(directory.join(format!("{case_id}-expected.accdb"))).unwrap();
                assert!(
                    expected == case.expected,
                    "expected={case_id}, first mismatch={:?}",
                    expected
                        .iter()
                        .zip(&case.expected)
                        .position(|(left, right)| left != right),
                );
                checked_expected += 1;

                let mut order = String::new();
                for entry in &case.new_stream {
                    let reference =
                        u32::from_be_bytes(entry[entry.len() - 4..].try_into().unwrap());
                    let row = case
                        .rows
                        .iter()
                        .find(|row| row.reference == reference)
                        .unwrap();
                    write_row(&mut order, row);
                }
                assert_eq!(
                    fs::read(directory.join(format!("{case_id}-order.tsv"))).unwrap(),
                    order.as_bytes(),
                );
                writeln!(
                    requests,
                    "{case_id}\t{source_id}\t{}\t{direction}\t{}\t{}",
                    case.name,
                    case.rows.len(),
                    case.new_counter,
                )
                .unwrap();
            }
        }
    }

    assert_eq!(checked_sources, 8);
    assert_eq!(checked_expected, 16);
    assert_eq!(
        fs::read(directory.join("requests.tsv")).unwrap(),
        requests.as_bytes()
    );
}
