use super::{expected, model};
use csv::ReaderBuilder;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[path = "../../support/mod.rs"]
mod cli_support;

static NEXT: AtomicU64 = AtomicU64::new(0);

const HEADER: [&str; 9] = [
    "Group ordinal",
    "Group kind",
    "Property ordinal",
    "Property",
    "Observation",
    "XML namespace",
    "Document ordinal",
    "Event",
    "Macro name",
];

struct Database {
    directory: PathBuf,
    path: PathBuf,
    original: Vec<u8>,
}

impl Database {
    fn new(original: Vec<u8>) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "tablecraft-observed-inventory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("source.accdb");
        fs::write(&path, &original).unwrap();
        Self {
            directory,
            path,
            original,
        }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        assert!(
            fs::read(&self.path).unwrap() == self.original,
            "source changed"
        );
        assert_eq!(fs::read_dir(&self.directory).unwrap().count(), 1);
        fs::remove_file(&self.path).unwrap();
        fs::remove_dir(&self.directory).unwrap();
    }
}

fn observation_rows(text: &str) -> Vec<Vec<String>> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut found = false;
    let mut rows = Vec::new();
    for row in reader.records() {
        let row = row.unwrap();
        if !found {
            if row.iter().eq(HEADER) {
                found = true;
            }
            continue;
        }
        if row.len() != HEADER.len() {
            break;
        }
        rows.push(row.iter().map(str::to_owned).collect());
    }
    assert!(found, "inventory section missing");
    rows
}

// This known fixture has no border characters or semantic whitespace in values.
// Concatenating each displayed column preserves header/value order while allowing
// the shared 120-column renderer to wrap either words or individual identifiers.
fn bordered_columns(text: &str) -> Vec<String> {
    let mut columns = vec![String::new(); HEADER.len()];
    let mut displayed_lines = 0;
    for line in text.lines() {
        let Some(interior) = line
            .strip_prefix('│')
            .and_then(|line| line.strip_suffix('│'))
        else {
            continue;
        };
        let cells = interior.split('┆').collect::<Vec<_>>();
        if cells.len() != HEADER.len() {
            continue;
        }
        for (column, cell) in columns.iter_mut().zip(cells) {
            column.extend(cell.chars().filter(|character| !character.is_whitespace()));
        }
        displayed_lines += 1;
    }
    assert!(displayed_lines > 1, "bordered inventory section missing");
    columns
}

#[test]
fn observed_macro_inventory_cli_reports_twenty_sources_without_body_text() {
    let mut commands = 0;
    for code in [5, 6] {
        for case in model::CASES {
            let source = model::source(code, case);
            let database = Database::new(source.bytes);
            let output = cli_support::run(&[
                "inspect",
                database.path(),
                "--table",
                "Items",
                "--extra",
                "--macro-inventory",
            ]);
            assert!(
                output.status.success(),
                "{case:?}/{code}: {:?}",
                output.stderr
            );
            assert!(output.stderr.is_empty());
            let text = String::from_utf8(output.stdout).unwrap();
            assert_eq!(observation_rows(&text), expected(case), "{case:?}/{code}");
            assert!(text.contains("Data-macro absence\tNot certified\n"));
            assert!(text.contains("Required macro coverage\tNot certified\n"));
            assert!(text.contains("Automatic execution\tNot authorized\n"));
            assert!(text.contains("Save permission\tNot granted\n"));
            let coverage = if matches!(
                case,
                model::Case::OpaqueExtra | model::Case::OpaqueGroup | model::Case::NonTextProperty
            ) {
                "Opaque content retained"
            } else {
                "Observed wide-MR2 framing"
            };
            assert!(text.contains(&format!("Structural coverage\t{coverage}\n")));
            assert!(!text.contains("<Statements"));
            assert!(!text.contains("FutureAction"));
            commands += 1;
        }
    }
    assert_eq!(commands, 20);
}

#[test]
fn observed_macro_inventory_cli_keeps_table_rendering_and_refuses_detail_conflicts() {
    let source = model::source(6, model::Case::EventAndNamed);
    let database = Database::new(source.bytes);
    let table = cli_support::run(&[
        "--format",
        "table",
        "inspect",
        database.path(),
        "--table",
        "Items",
        "--extra",
        "--macro-inventory",
    ]);
    assert!(table.status.success(), "{:?}", table.stderr);
    assert!(table.stderr.is_empty());
    let text = String::from_utf8(table.stdout).unwrap();
    assert!(
        text.len() <= 16 * 1024,
        "bounded fixture report unexpectedly large"
    );
    let rows = expected(model::Case::EventAndNamed);
    let expected_columns = (0..HEADER.len())
        .map(|column| {
            std::iter::once(HEADER[column])
                .chain(rows.iter().map(|row| row[column]))
                .flat_map(str::chars)
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert_eq!(bordered_columns(&text), expected_columns);
    assert!(!text.contains("<Statements"));
    assert!(!text.contains("FutureAction"));

    for other in ["--macro-program", "--macro-xml"] {
        let refused = cli_support::run(&[
            "inspect",
            "missing-inventory-source.accdb",
            "--table",
            "Items",
            "--extra",
            "--macro-inventory",
            other,
        ]);
        assert!(!refused.status.success());
        assert!(refused.stdout.is_empty());
        assert!(
            String::from_utf8(refused.stderr)
                .unwrap()
                .contains("cannot be used with")
        );
    }
    let missing = cli_support::run(&[
        "inspect",
        "missing-inventory-source.accdb",
        "--macro-inventory",
    ]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("--extra")
    );

    let limited = cli_support::run(&[
        "inspect",
        database.path(),
        "--table",
        "Items",
        "--extra",
        "--macro-inventory",
        "--native-steps",
        "1",
    ]);
    assert!(!limited.status.success());
    assert!(limited.stdout.is_empty());
}
