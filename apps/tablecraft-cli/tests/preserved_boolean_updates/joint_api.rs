//! Literal joint requests through existing typed and SQL copy commands.
use super::joint_model as model;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;

fn kind(column: model::Column, assignment: &model::Assignment) -> &'static str {
    let carrier = if assignment.expected != model::Cell::Null {
        assignment.expected
    } else {
        assignment.replacement
    };
    match carrier {
        model::Cell::Boolean(_) => "boolean",
        model::Cell::Int32(_) => "int32",
        model::Cell::Int64(_) => "int64",
        model::Cell::Currency(_) => "currency-scaled",
        model::Cell::Date(_) => "date-bits",
        model::Cell::Null => match column.native() {
            4 => "int32",
            19 => "int64",
            5 => "currency-scaled",
            8 => "date-bits",
            _ => unreachable!("closed nullable scalar source"),
        },
    }
}

fn encoded(cell: model::Cell) -> String {
    match cell.value() {
        Value::Null => "null".to_owned(),
        Value::Boolean(value) => value.to_string(),
        Value::Int32(value) => value.to_string(),
        Value::Int64(value) => value.to_string(),
        Value::Currency(value) => value.scaled().to_string(),
        Value::DateTime(value) => format!("{:016x}", value.to_bits()),
        _ => unreachable!("closed fixed scalar source carriers"),
    }
}

fn typed(groups: &[model::Group], source: &Path, output: &Path) -> Output {
    assert!(groups.iter().all(|group| !group.updates.is_empty()));
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["write", "preview-update"])
        .arg(source)
        .arg(output);
    for group in groups {
        for assignment in &group.updates {
            let expected = encoded(assignment.expected);
            let replacement = encoded(assignment.replacement);
            command.args([
                "--set",
                group.table,
                group.name,
                &assignment.row.to_string(),
                kind(group.column, assignment),
                &expected,
                &replacement,
            ]);
        }
    }
    command.output().unwrap()
}

fn sql(source: &Path, output: &Path, statement: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", statement, "--database"])
        .arg(source)
        .arg("--output")
        .arg(output)
        .output()
        .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (old, new))| old != new)
        .take(8)
        .map(|(offset, (old, new))| (offset, *old, *new))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "First byte differences: {differences:?}"
    );
}

#[test]
fn typed_joint_cli_matches_thirty_two_complete_images_in_both_group_orders() {
    let mut copies = 0;
    let mut unrepresentable = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        model::assert_complement(&case);
        if case.groups.iter().any(|group| group.updates.is_empty()) {
            // The typed protocol transmits assignments and cannot declare an
            // empty group. Do not omit targets or fabricate no-op assignments.
            unrepresentable += 1;
            continue;
        }
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        fs::write(&source, &case.before).unwrap();
        for reverse in [false, true] {
            let output = directory.path().join(if reverse {
                "reverse.accdb"
            } else {
                "forward.accdb"
            });
            let mut groups = case.groups.clone();
            if reverse {
                groups.reverse();
            }
            let result = typed(&groups, &source, &output);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            assert_image(&fs::read(&output).unwrap(), expected);
            assert_image(&fs::read(&source).unwrap(), &case.before);
            let report = String::from_utf8(result.stdout).unwrap();
            assert!(report.contains(&format!("Candidate rows\t{}\n", case.changed)));
            copies += 1;
        }
    }
    assert_eq!((copies, unrepresentable), (32, 3));
}

#[test]
fn sql_joint_cli_uses_literal_conversions_and_true_empty_groups() {
    for (recipe, statement, matched) in [
        (
            model::Recipe::BooleanAndLarge,
            "UPDATE [Items] SET [Yes_Field]=True,[Large_Number]=CLngLng(-101) WHERE [ID]=1",
            1,
        ),
        (
            model::Recipe::ExactNoops,
            "UPDATE [Items] SET [Large_Number]=CLngLng(1000),[Currency_Field]=CCur(1.2345),[Date_Field]=#1899-12-30#,[Yes_Field]=False WHERE [ID]=1",
            1,
        ),
        (
            model::Recipe::SharedHighMask,
            "UPDATE [Items] SET [Currency_Field]=NULL,[Yes_Field]=True,[No_Field]=True WHERE [ID]=1",
            1,
        ),
        (
            model::Recipe::EntirelyEmpty,
            "UPDATE [Items] SET [Large_Number]=[Large_Number],[Date_Field]=[Date_Field],[Currency_Field]=[Currency_Field],[Yes_Field]=[Yes_Field],[No_Field]=[No_Field] WHERE [ID]=-1",
            0,
        ),
    ] {
        let case = model::source(6, recipe);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        let output = directory.path().join("candidate.accdb");
        fs::write(&source, &case.before).unwrap();
        let result = sql(&source, &output, statement);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_image(&fs::read(&output).unwrap(), case.expected.as_ref().unwrap());
        assert_image(&fs::read(&source).unwrap(), &case.before);
        let report = String::from_utf8(result.stdout).unwrap();
        assert!(report.contains(&format!(
            "Matched rows\t{matched}\nCandidate rows\t{}\n",
            case.changed
        )));
    }
}

#[test]
fn representable_joint_cli_refusals_never_publish_and_preserve_source() {
    let mut refusals = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        if case.refusal.is_none()
            || matches!(
                recipe,
                model::Recipe::DuplicateColumn | model::Recipe::EmptyIneligible
            )
        {
            // Same-name assignments are merged by the typed protocol. It does
            // not transmit duplicate native groups or declarations of empties.
            continue;
        }
        assert!(case.expected.is_none());
        if let Some(patch) = case.required_patch {
            assert_eq!(case.before[patch], 1);
            for row in 0..3 {
                assert!(matches!(
                    model::read(&case.before, model::Column::Currency, row),
                    model::Cell::Currency(_)
                ));
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        fs::write(&source, &case.before).unwrap();
        for reverse in [false, true] {
            let output = directory.path().join(if reverse {
                "reverse.accdb"
            } else {
                "forward.accdb"
            });
            let mut groups = case.groups.clone();
            if reverse {
                groups.reverse();
            }
            let result = typed(&groups, &source, &output);
            assert!(!result.status.success(), "{recipe:?}");
            assert!(result.stdout.is_empty());
            assert!(!output.exists());
            assert_image(&fs::read(&source).unwrap(), &case.before);
            refusals += 1;
        }
    }
    assert_eq!(refusals, 12);
    let case = model::source(6, model::Recipe::LateRequired);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let output = directory.path().join("candidate.accdb");
    fs::write(&source, &case.before).unwrap();
    let result = sql(
        &source,
        &output,
        "UPDATE [Items] SET [Yes_Field]=True,[Currency_Field]=NULL WHERE [ID]=1",
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!output.exists());
    assert_image(&fs::read(&source).unwrap(), &case.before);
}

#[test]
fn joint_cli_output_conflicts_preserve_existing_bytes() {
    let case = model::source(6, model::Recipe::BooleanAndLarge);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let existing = directory.path().join("existing.accdb");
    fs::write(&source, &case.before).unwrap();
    fs::write(&existing, b"existing destination").unwrap();
    for output in [&source, &existing] {
        let result = typed(&case.groups, &source, output);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert_image(&fs::read(&source).unwrap(), &case.before);
        assert_eq!(fs::read(&existing).unwrap(), b"existing destination");
    }
}
