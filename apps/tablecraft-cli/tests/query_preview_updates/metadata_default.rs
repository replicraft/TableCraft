//! Real command consumers of the unchanged joint-record literal source owners.
#[path = "metadata_default/growing.rs"]
mod growing;
use super::{budget, fixture as base, fs, rows, run};
use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, check_table, check_table_constraints};
use tablecraft_model::CatalogScope;

#[path = "../../../../crates/engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/capacity.rs"]
mod capacity;
#[path = "../../../../crates/engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/model.rs"]
mod model;

fn qualify(bytes: &[u8], expected: &[model::Row], family: model::Family) {
    assert_eq!(
        rows(bytes),
        expected
            .iter()
            .map(|row| model::values(row, family))
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
    assert_eq!(checked.rows(), 4);
    assert_eq!(checked.indexes().len(), 6);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
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
        4
    );
}

fn preservation(case: &model::Case) {
    assert_eq!(case.before.len(), case.expected.len());
    assert_eq!(
        &case.before[..12 * model::PAGE],
        &case.expected[..12 * model::PAGE]
    );
    assert_eq!(case.records.len(), case.final_records.len());
    assert_eq!(case.records.len(), 4);
    for (ordinal, (before, after)) in case.records.iter().zip(&case.final_records).enumerate() {
        assert_eq!(before.len(), after.len());
        if !case.changed.contains(&ordinal) {
            assert_eq!(before, after);
        }
    }
    if case.forwarded {
        assert_eq!(case.before.len(), 18 * model::PAGE);
    }
}

#[test]
fn qualify_cli_metadata_default_sources_before_command_preview() {
    for code in [5, 6] {
        for family in [
            model::Family::Legacy,
            model::Family::Modern,
            model::Family::Binary,
        ] {
            for descending in [false, true] {
                for recipe in [
                    model::Recipe::Permutation,
                    model::Recipe::Shorter,
                    model::Recipe::Nulls,
                ] {
                    let case = model::case(code, family, descending, recipe, false);
                    qualify(&case.before, &case.rows, case.family);
                    qualify(&case.expected, &case.final_rows, case.family);
                    preservation(&case);
                }
            }
        }
    }
    for family in [
        model::Family::Legacy,
        model::Family::Modern,
        model::Family::Binary,
    ] {
        let case = model::case(6, family, true, model::Recipe::Permutation, true);
        qualify(&case.before, &case.rows, case.family);
        qualify(&case.expected, &case.final_rows, case.family);
        preservation(&case);
    }
    for family in [model::Family::Legacy, model::Family::Modern] {
        for descending in [false, true] {
            let case = capacity::case(family, descending);
            qualify(&case.before, &case.rows, case.family);
            qualify(&case.expected, &case.final_rows, case.family);
            preservation(&case);
        }
    }
}

#[test]
fn default_sql_joint_updates_publish_exact_frozen_images_without_new_flags() {
    for family in [model::Family::Legacy, model::Family::Modern] {
        for descending in [false, true] {
            for case in [
                model::case(6, family, descending, model::Recipe::Permutation, false),
                capacity::case(family, descending),
            ] {
                let directory = tempfile::tempdir().unwrap();
                let input = directory.path().join("before.accdb");
                let output = directory.path().join("after.accdb");
                fs::write(&input, &case.before).unwrap();
                let result = run(
                    "UPDATE Items SET Age=IIf(Id=1,20,10), Name=IIf(Id=1,'é','a') WHERE Id IN(1,2)",
                    &input,
                    &output,
                    &[],
                );
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(fs::read(&input).unwrap(), case.before);
                assert_eq!(fs::read(&output).unwrap(), case.expected);
            }
        }
    }
}

#[test]
fn default_single_variable_matches_explicit_route_and_multiple_variable_refuses() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    let default = directory.path().join("default.accdb");
    let explicit = directory.path().join("explicit.accdb");
    let refused = directory.path().join("refused.accdb");
    let before = super::source::source();
    fs::write(&input, &before).unwrap();
    let statement = "UPDATE Items SET Label='m' WHERE Id=1";
    for (output, flags) in [(&default, &[][..]), (&explicit, &["--variable"][..])] {
        let result = run(statement, &input, output, flags);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    // Dispatch consistency, separate from the independent whole-image goldens.
    assert_eq!(fs::read(default).unwrap(), fs::read(explicit).unwrap());
    let result = run(
        "UPDATE Items SET Label='m', Other='n' WHERE FALSE",
        &input,
        &refused,
        &[],
    );
    assert!(!result.status.success());
    assert!(!refused.exists());
    assert_eq!(fs::read(input).unwrap(), before);
}
