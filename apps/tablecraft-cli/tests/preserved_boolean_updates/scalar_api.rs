//! Existing typed and SQL commands against the one frozen scalar source owner.
use super::scalar_model::{self as model, Recipe, Target};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;

fn encode(target: Target, bytes: Option<[u8; 8]>) -> (&'static str, String) {
    let kind = match target {
        Target::LargeNumber => "int64",
        Target::Currency => "currency-scaled",
        Target::Date | Target::Time => "date-bits",
    };
    let value = match target.value(bytes) {
        Value::Null => "null".to_owned(),
        Value::Int64(value) => value.to_string(),
        Value::Currency(value) => value.scaled().to_string(),
        Value::DateTime(value) => format!("{:016x}", value.to_bits()),
        _ => unreachable!("closed independent scalar model"),
    };
    (kind, value)
}

fn typed(case: &model::Case, source: &Path, output: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["write", "preview-update"])
        .arg(source)
        .arg(output);
    for row in 0..case.requested_rows {
        let (kind, before) = encode(case.target, case.initial[row]);
        let (_, after) = encode(case.target, case.replacement[row]);
        command.args([
            "--set",
            "Items",
            case.target.name(),
            &row.to_string(),
            kind,
            &before,
            &after,
        ]);
    }
    command.output().unwrap()
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
fn typed_scalar_commands_match_thirty_five_complete_images() {
    let mut comparisons = 0;
    for (target, recipe) in model::cases() {
        let case = model::source(6, target, recipe);
        model::assert_complement(&case);
        let Some(expected) = &case.expected else {
            continue;
        };
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        let output = directory.path().join("candidate.accdb");
        fs::write(&source, &case.before).unwrap();
        let result = typed(&case, &source, &output);
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
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
        comparisons += 1;
    }
    assert_eq!(comparisons, 35);
}

#[test]
fn explicit_sql_scalar_conversions_match_four_complete_images() {
    for (target, recipe, expression) in [
        (Target::LargeNumber, Recipe::Change, "CLngLng(-101)"),
        (Target::Date, Recipe::Change, "#1899-12-31 06:00:00#"),
        (Target::Time, Recipe::ValueToNull, "NULL"),
        (Target::Currency, Recipe::Change, "CCur(-1.0001)"),
    ] {
        let case = model::source(6, target, recipe);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        let output = directory.path().join("candidate.accdb");
        fs::write(&source, &case.before).unwrap();
        let sql = format!(
            "UPDATE [Items] SET [{}]={expression} WHERE [ID]=1",
            target.name()
        );
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "preview-update", &sql, "--database"])
            .arg(&source)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_image(&fs::read(&output).unwrap(), case.expected.as_ref().unwrap());
        assert_image(&fs::read(&source).unwrap(), &case.before);
        let report = String::from_utf8(result.stdout).unwrap();
        assert!(report.contains("Matched rows\t1\nCandidate rows\t1\n"));
    }
}

#[test]
fn header_five_and_required_scalar_refusals_create_no_output() {
    let mut refused = 0;
    for code in [5, 6] {
        for (target, recipe) in model::cases() {
            if code == 6 && recipe != Recipe::RequiredRefusal {
                continue;
            }
            let case = model::source(code, target, recipe);
            if recipe == Recipe::RequiredRefusal {
                assert!(case.initial.iter().all(Option::is_some));
                assert_eq!(case.before[case.required_patch.unwrap()], 1);
            }
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("source.accdb");
            let output = directory.path().join("candidate.accdb");
            fs::write(&source, &case.before).unwrap();
            let result = typed(&case, &source, &output);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!output.exists());
            assert_image(&fs::read(&source).unwrap(), &case.before);
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            refused += 1;
        }
    }
    assert_eq!(refused, 43);
}

#[test]
fn scalar_output_conflicts_preserve_original_and_existing_destination() {
    let case = model::source(6, Target::Currency, Recipe::Change);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let existing = directory.path().join("existing.accdb");
    fs::write(&source, &case.before).unwrap();
    fs::write(&existing, b"existing destination").unwrap();
    for output in [&source, &existing] {
        let result = typed(&case, &source, output);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert_image(&fs::read(&source).unwrap(), &case.before);
        assert_eq!(fs::read(&existing).unwrap(), b"existing destination");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
