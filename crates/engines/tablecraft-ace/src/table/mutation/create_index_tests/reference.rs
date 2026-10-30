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
