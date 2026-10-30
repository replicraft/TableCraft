//! SQL and typed commands share one actual joint-growth source owner.
use super::{base, budget, fs, rows, run};
use std::{
    io::Cursor,
    path::Path,
    process::{Command, Output},
};
use tablecraft_ace::{
    IndexCheckStatus, check_table, check_table_constraints, inspect_table_index_counter,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[path = "../../../../../crates/engines/tablecraft-ace/src/table/mutation/fitted/growing/joint_split_tests/model.rs"]
mod source_model;
use source_model::{Family, Shape};

fn cases() -> Vec<source_model::Case> {
    let mut cases = Vec::new();
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for shape in [Shape::Root, Shape::Branch] {
                    cases.push(source_model::case(code, family, descending, shape));
                }
            }
        }
    }
    cases
}

fn name(row: &source_model::Row, family: Family) -> Value {
    match &row.name {
        None => Value::Null,
        Some(bytes) if matches!(family, Family::Binary) => Value::Binary(bytes.clone()),
        Some(bytes) => Value::Text(String::from_utf8(bytes.clone()).unwrap()),
    }
}

fn qualify(bytes: &[u8], expected: &[source_model::Row], family: Family) {
    assert_eq!(
        rows(bytes),
        expected
            .iter()
            .map(|row| vec![
                Value::Int32(row.id),
                Value::Int32(row.age),
                name(row, family)
            ])
            .collect::<Vec<_>>()
    );
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 64);
    assert_eq!(checked.indexes().len(), 4);
    for index in checked.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        let counter = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            index.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            counter.value(),
            if index.index().data_number() == 0 {
                701
            } else {
                901
            }
        );
    }
    assert_eq!(
        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        64
    );
}

#[test]
fn qualify_joint_index_frontend_sources_and_complete_expected_before_actions() {
    let cases = cases();
    assert_eq!(cases.len(), 24);
    for case in cases {
        qualify(&case.before, &case.rows, case.family);
        qualify(&case.expected, &case.final_rows, case.family);
        assert_eq!(&case.before[..4096], &case.expected[..4096]);
        assert!(!case.data_pages.is_empty());
        assert!(!case.index_pages.is_empty());
        assert!(
            case.data_pages
                .iter()
                .all(|page| !case.index_pages.contains(page))
        );
        assert_eq!(
            case.expected.len(),
            case.before.len() + (case.data_pages.len() + case.index_pages.len()) * 4096
        );
    }
}

fn sql(case: &source_model::Case) -> String {
    let first = String::from_utf8(case.final_rows[0].name.clone().unwrap()).unwrap();
    let second = String::from_utf8(case.final_rows[1].name.clone().unwrap()).unwrap();
    format!("UPDATE Items SET Age=Age+1000,Name=IIf(Id Mod 2=1,'{first}','{second}')")
}

fn typed(case: &source_model::Case, before: &Path, after: &Path, stale: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["write", "preview-update"])
        .arg(before)
        .arg(after)
        .args(["--variable", "--index-growth"]);
    for (ordinal, (original, final_row)) in case.rows.iter().zip(&case.final_rows).enumerate() {
        let expected = if stale && ordinal == case.rows.len() - 1 {
            original.age - 1
        } else {
            original.age
        };
        command.args([
            "--set",
            "Items",
            "Age",
            &ordinal.to_string(),
            "int32",
            &expected.to_string(),
            &final_row.age.to_string(),
        ]);
        let encode = |value: &Option<Vec<u8>>| match value {
            None => "null".to_owned(),
            Some(bytes) if matches!(case.family, Family::Binary) => {
                bytes.iter().map(|byte| format!("{byte:02x}")).collect()
            }
            Some(bytes) => String::from_utf8(bytes.clone()).unwrap(),
        };
        command.args([
            "--set",
            "Items",
            "Name",
            &ordinal.to_string(),
            if matches!(case.family, Family::Binary) {
                "binary"
            } else {
                "text"
            },
            &encode(&original.name),
            &encode(&final_row.name),
        ]);
    }
    command.output().unwrap()
}

#[test]
fn joint_index_sql_flags_publish_16_complete_text_images() {
    for case in cases()
        .into_iter()
        .filter(|case| !matches!(case.family, Family::Binary))
    {
        let directory = tempfile::tempdir().unwrap();
        let before = directory.path().join("before.accdb");
        let after = directory.path().join("after.accdb");
        fs::write(&before, &case.before).unwrap();
        let result = run(
            &sql(&case),
            &before,
            &after,
            &["--variable", "--index-growth"],
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_eq!(fs::read(&after).unwrap(), case.expected);
        assert_eq!(fs::read(&before).unwrap(), case.before);
    }
}

#[test]
fn joint_index_typed_flags_preserve_all_24_expected_images_including_binary() {
    for case in cases() {
        let directory = tempfile::tempdir().unwrap();
        let before = directory.path().join("before.accdb");
        let after = directory.path().join("after.accdb");
        fs::write(&before, &case.before).unwrap();
        let result = typed(&case, &before, &after, false);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_eq!(fs::read(&after).unwrap(), case.expected);
        assert_eq!(fs::read(&before).unwrap(), case.before);
    }
}

#[test]
fn joint_index_stale_input_and_no_clobber_never_publish_partial_copies() {
    for family in [Family::Legacy, Family::Binary] {
        let case = source_model::case(6, family, true, Shape::Branch);
        let directory = tempfile::tempdir().unwrap();
        let before = directory.path().join("before.accdb");
        let after = directory.path().join("after.accdb");
        fs::write(&before, &case.before).unwrap();
        let result = typed(&case, &before, &after, true);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!after.exists());
        assert_eq!(fs::read(&before).unwrap(), case.before);

        fs::write(&after, b"existing destination").unwrap();
        let result = typed(&case, &before, &after, false);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert_eq!(fs::read(&after).unwrap(), b"existing destination");
        assert_eq!(fs::read(&before).unwrap(), case.before);

        let result = typed(&case, &before, &before, false);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert_eq!(fs::read(&before).unwrap(), case.before);
    }
}
