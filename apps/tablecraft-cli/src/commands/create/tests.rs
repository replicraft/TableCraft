use super::*;
use crate::{Cli, commands::Command};
use clap::Parser;
use std::{fs, io, path::Path};
use tablecraft_access::creation::CreationError;
use tablecraft_io::publication::PublicationError;
use tablecraft_testkit::access::canonical_creation::{self, Case};

fn arguments(path: &Path, table: &str, columns: &[String], data: Option<&Path>) -> Arguments {
    let mut words = vec![
        "tablecraft".to_owned(),
        "create".to_owned(),
        path.to_str().unwrap().to_owned(),
        "--table".to_owned(),
        table.to_owned(),
    ];
    for column in columns {
        words.extend(["--column".to_owned(), column.clone()]);
    }
    if let Some(data) = data {
        words.extend(["--data".to_owned(), data.to_str().unwrap().to_owned()]);
    }
    match Cli::try_parse_from(words).unwrap().command.unwrap() {
        Command::Create(arguments) => arguments,
        _ => panic!("expected the actual create command"),
    }
}

fn data(columns: &[String], rows: &[Vec<i32>]) -> String {
    let mut text = columns.join("\t");
    text.push('\n');
    for row in rows {
        text.push_str(
            &row.iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join("\t"),
        );
        text.push('\n');
    }
    text
}

#[test]
fn six_fresh_files_match_complete_independent_sources_and_prepared_reports() {
    for case in canonical_creation::CASES {
        let source = canonical_creation::source(case);
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new.accdb");
        let input = root.path().join("rows.tsv");
        let input_path = if case == Case::Empty {
            None
        } else {
            fs::write(&input, data(&source.columns, &source.rows)).unwrap();
            Some(input.as_path())
        };
        let mut report = Vec::new();
        execute(
            arguments(&output, source.table, &source.columns, input_path),
            None,
            &mut report,
        )
        .unwrap();
        assert_eq!(fs::read(&output).unwrap(), source.bytes);
        assert_eq!(
            String::from_utf8(report).unwrap(),
            format!(
                "Database\tCreated ACE 2016\nColumns\t{}\nRows\t{}\nDurable output\tFile and captured-directory barriers completed\n",
                source.columns.len(),
                source.rows.len(),
            )
        );
        assert_eq!(
            fs::read_dir(root.path()).unwrap().count(),
            1 + usize::from(input_path.is_some())
        );
    }
}

#[test]
fn invalid_headers_cells_shapes_and_passwords_publish_no_file() {
    let columns = vec!["Id".to_owned(), "Score".to_owned()];
    let too_many = data(&columns, &vec![vec![1, 10]; 65]);
    for text in [
        "Score\tId\n1\t10\n",
        "Id\tScore\n1\tNULL\n",
        "Id\tScore\n1\t+10\n",
        "Id\tScore\n1\t2147483648\n",
        "Id\tScore\n1\n",
        "Id\tScore\n1\t\"10",
        &too_many,
    ] {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new.accdb");
        let input = root.path().join("rows.tsv");
        fs::write(&input, text).unwrap();
        let mut report = Vec::new();
        let error = execute(
            arguments(&output, "Items", &columns, Some(&input)),
            None,
            &mut report,
        )
        .unwrap_err();
        assert!(matches!(error, Error::Input(_)));
        assert!(report.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("absent-parent/new.accdb");
    let absent_input = root.path().join("absent.tsv");
    assert!(matches!(
        execute(
            arguments(&output, "Items", &columns, Some(&absent_input)),
            Some("secret"),
            &mut Vec::new(),
        ),
        Err(Error::Password)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn quoted_tsv_and_header_only_empty_input_are_explicitly_supported() {
    let root = tempfile::tempdir().unwrap();
    let source = canonical_creation::source(Case::Named);
    let input = root.path().join("rows.tsv");
    fs::write(&input, "\"Person Id\"\t\"Score Δ\"\r\n\"7\"\t\"42\"\r\n").unwrap();
    let output = root.path().join("named.accdb");
    execute(
        arguments(&output, source.table, &source.columns, Some(&input)),
        None,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(fs::read(output).unwrap(), source.bytes);
    let source = canonical_creation::source(Case::Empty);
    fs::write(&input, "Id\tScore\n").unwrap();
    let output = root.path().join("empty.accdb");
    execute(
        arguments(&output, source.table, &source.columns, Some(&input)),
        None,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(fs::read(output).unwrap(), source.bytes);
}

#[test]
fn existing_files_directories_and_symlinks_are_never_overwritten() {
    use std::os::unix::fs::symlink;
    let source = canonical_creation::source(Case::Empty);
    for kind in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("existing");
        let target = root.path().join("other");
        match kind {
            0 => fs::write(&output, b"retained").unwrap(),
            1 => fs::create_dir(&output).unwrap(),
            2 => {
                fs::write(&target, b"retained").unwrap();
                symlink(&target, &output).unwrap();
            }
            _ => unreachable!(),
        }
        let initial_entries = fs::read_dir(root.path()).unwrap().count();
        let error = execute(
            arguments(&output, source.table, &source.columns, None),
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            Error::Creation(CreationError::Publication(
                PublicationError::NotPublished { .. }
            ))
        ));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), initial_entries);
        if kind == 1 {
            assert!(output.is_dir());
        } else {
            assert_eq!(fs::read(&output).unwrap(), b"retained");
        }
    }
}

struct FailedOutput {
    fail_write: bool,
}

impl Write for FailedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(bytes.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn stdout_write_and_flush_failures_retain_acknowledged_publication() {
    let source = canonical_creation::source(Case::Empty);
    for fail_write in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new.accdb");
        let error = execute(
            arguments(&output, source.table, &source.columns, None),
            None,
            &mut FailedOutput { fail_write },
        )
        .unwrap_err();
        assert_eq!(fs::read(&output).unwrap(), source.bytes);
        assert!(!error.invalid_input());
        match error {
            Error::Output { receipt, cause } => {
                assert!(receipt.namespace_synced());
                assert_eq!((receipt.row_count(), receipt.column_count()), (0, 2));
                assert_eq!(cause.kind(), io::ErrorKind::BrokenPipe);
            }
            _ => panic!("known publication must survive failed output"),
        }
    }
}

#[test]
fn report_allocation_refusal_precedes_publication_and_resources_are_not_bad_input() {
    use tablecraft_ace::CreateDatabaseError;
    let source = canonical_creation::source(Case::Empty);
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("new.accdb");
    let mut budget = ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000);
    let destination = CreationDestination::capture(&output, &mut budget, &|| false).unwrap();
    let columns: Vec<_> = source.columns.iter().map(String::as_str).collect();
    let prepared = destination
        .prepare(source.table, &columns, &[], &mut budget, &|| false)
        .unwrap();
    assert!(report::Report::prepare(&prepared, &mut ResourceBudget::new(0, 0, 0)).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    for cause in [
        CreateDatabaseError::Inspect(InspectError::Cancelled),
        CreateDatabaseError::Layout,
    ] {
        assert!(!Error::Creation(CreationError::Create(cause)).invalid_input());
    }
}
