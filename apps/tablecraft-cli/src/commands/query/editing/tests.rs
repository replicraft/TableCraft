//! A real Jet 4 plan reaches the common publisher once, after source release.
use super::{Error, publication, run_with_publisher};
use crate::commands::rows::stream::tests::fixture;
use std::{cell::Cell, fs, io::Cursor};
use tablecraft_ace::{IndexCheckStatus, RuleCheckStatus, check_table, check_table_constraints};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::ExclusiveFile;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000)
}

fn qualify(bytes: &[u8]) {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(constraints.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.relationships().is_empty());
}

fn source_and_expected() -> (Vec<u8>, Vec<u8>) {
    let before = fixture::with_rule("ValidationRule", ">=0 And <=100");
    let mut expected = before.clone();
    let page = 12 * 4096;
    let start = usize::from(u16::from_le_bytes(
        before[page + 16..page + 18].try_into().unwrap(),
    )) & 0x1fff;
    let offset = page + start + 6;
    assert_eq!(&before[offset..offset + 4], &20_i32.to_le_bytes());
    expected[offset..offset + 4].copy_from_slice(&27_i32.to_le_bytes());
    qualify(&before);
    qualify(&expected);
    (before, expected)
}

#[test]
fn jet4_plan_publishes_exact_expected_bytes_once_after_source_unlock() {
    let (before, expected) = source_and_expected();
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.mdb");
    let output = directory.path().join("copy.mdb");
    fs::write(&input, &before).unwrap();
    let query = tablecraft_sql::parse_query(
        "UPDATE Items SET Score=27 WHERE Id=2",
        tablecraft_sql::ParseLimits::default(),
    )
    .unwrap();
    let calls = Cell::new(0);
    let report = run_with_publisher(
        query.query(),
        &input,
        &output,
        None,
        |path, bytes, budget| {
            calls.set(calls.get() + 1);
            assert_eq!(path, output);
            assert_eq!(bytes, expected);
            assert!(!output.exists());
            ExclusiveFile::open(&input).unwrap().close().unwrap();
            let remaining = budget.remaining(Resource::Bytes);
            publication::publish(path, bytes, budget, &|| false)?;
            assert_eq!(
                budget.remaining(Resource::Bytes),
                remaining - bytes.len() as u64
            );
            Ok(())
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(calls.get(), 1);
    assert_eq!(report, "Matched rows\t1\nChanged rows\t1\n");
    assert_eq!(fs::read(&input).unwrap(), before);
    assert_eq!(fs::read(&output).unwrap(), expected);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn jet4_publication_errors_propagate_without_retry_or_visible_output() {
    let (before, expected) = source_and_expected();
    let query = tablecraft_sql::parse_query(
        "UPDATE Items SET Score=27 WHERE Id=2",
        tablecraft_sql::ParseLimits::default(),
    )
    .unwrap();
    for failure in [0, 1, 2] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source.mdb");
        let output = directory.path().join("copy.mdb");
        fs::write(&input, &before).unwrap();
        let calls = Cell::new(0);
        let result = run_with_publisher(query.query(), &input, &output, None, |path, bytes, _| {
            calls.set(calls.get() + 1);
            assert_eq!(path, output);
            assert_eq!(bytes, expected);
            ExclusiveFile::open(&input).unwrap().close().unwrap();
            Err(match failure {
                0 => publication::Error::Write(tablecraft_io::IoError::Io(
                    std::io::ErrorKind::WriteZero,
                )),
                1 => publication::Error::Sync(std::io::ErrorKind::Other),
                _ => publication::Error::Persist(std::io::ErrorKind::PermissionDenied),
            })
        });
        match (failure, result) {
            (
                0,
                Err(Error::Copy(publication::Error::Write(tablecraft_io::IoError::Io(
                    std::io::ErrorKind::WriteZero,
                )))),
            )
            | (1, Err(Error::Copy(publication::Error::Sync(std::io::ErrorKind::Other))))
            | (
                2,
                Err(Error::Copy(publication::Error::Persist(std::io::ErrorKind::PermissionDenied))),
            ) => {}
            _ => panic!("publication failure class changed for case {failure}"),
        }
        assert_eq!(calls.get(), 1);
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), before);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
