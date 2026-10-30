//! Real commands follow the independent whole-image SOURCE gate.
use super::model;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;

fn same_image(actual: &[u8], expected: &[u8]) {
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    let differences = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
    assert!(
        actual == expected,
        "whole image lengths {}/{}, first {first:?}, differences {differences}",
        actual.len(),
        expected.len()
    );
}

fn sql(statement: &str, source: &Path, output: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", statement, "--database"])
        .arg(source)
        .arg("--output")
        .arg(output)
        .args(flags)
        .output()
        .unwrap()
}

#[test]
fn sql_default_publishes8_two_text_whole_images_with_optional_fixed_siblings() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (recipe, statement) in [
                (
                    model::Recipe::Permutation,
                    "UPDATE Items SET [First]=IIf(Id=1,'s','a'), [Second]=IIf(Id=1,'s','a') WHERE Id IN(1,2)",
                ),
                (
                    model::Recipe::NullShrinkGrow,
                    "UPDATE Items SET [First]=IIf(Id=3,'',IIf(Id=4,'a',[First])), [Second]=IIf(Id=3,NULL,IIf(Id=4,'aa',IIf(Id=6,'',[Second]))), Age=IIf(Id=3,22,IIf(Id=5,NULL,Age)) WHERE Id IN(3,4,5,6)",
                ),
            ] {
                let case = model::fitted(code, model::Family::LegacyText, descending, recipe);
                let directory = tempfile::tempdir().unwrap();
                let source = directory.path().join("before.accdb");
                let output = directory.path().join("after.accdb");
                fs::write(&source, &case.before).unwrap();
                let result = sql(statement, &source, &output, &[]);
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                same_image(&fs::read(output).unwrap(), &case.expected);
                same_image(&fs::read(&source).unwrap(), &case.before);
            }
        }
    }
}

fn value(value: &Value) -> (&'static str, String) {
    match value {
        Value::Null => ("binary", "null".into()),
        Value::Int32(number) => ("int32", number.to_string()),
        Value::Text(text) => ("text", format!("value:{text}")),
        Value::Binary(bytes) => (
            "binary",
            bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        ),
        _ => panic!("unselected fixture carrier"),
    }
}

fn typed(case: &model::Case, source: &Path, output: &Path, stale: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["write", "preview-update"])
        .arg(source)
        .arg(output)
        .arg("--variable");
    if matches!(case.placement, model::Placement::DataIndexes) {
        command.arg("--index-growth");
    }
    for (column, name, rows) in [
        (1, "Age", &case.fixed_rows),
        (2, "First", &case.first_rows),
        (3, "Second", &case.second_rows),
    ] {
        for &row in rows {
            let before = model::values(&case.rows[row], case.family);
            let after = model::values(&case.final_rows[row], case.family);
            let (mut kind, mut expected) = value(&before[column]);
            let (replacement_kind, replacement) = value(&after[column]);
            if matches!(before[column], Value::Null) {
                kind = replacement_kind;
            }
            if stale && column == 3 && row == case.rows.len() - 1 {
                expected = "00".into();
            }
            command.args([
                "--set",
                "Items",
                name,
                &row.to_string(),
                kind,
                &expected,
                &replacement,
            ]);
        }
    }
    command.output().unwrap()
}

#[test]
fn typed_modes_publish12_binary_joint_images_including_variable_only() {
    let mut cases = Vec::new();
    for code in [5, 6] {
        for family in [model::Family::LegacyBinary, model::Family::ModernBinary] {
            for indexes in [false, true] {
                cases.push(model::growing(code, family, indexes));
            }
        }
        for descending in [false, true] {
            cases.push(model::fitted(
                code,
                model::Family::Binary,
                descending,
                model::Recipe::Permutation,
            ));
        }
    }
    assert_eq!(cases.len(), 12);
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("before.accdb");
        let output = directory.path().join("after.accdb");
        fs::write(&source, &case.before).unwrap();
        let result = typed(&case, &source, &output, false);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        same_image(&fs::read(output).unwrap(), &case.expected);
        same_image(&fs::read(&source).unwrap(), &case.before);
    }
}

#[test]
fn late_typed_stale_and_unsupported_modes_leave_no_copy() {
    let case = model::growing(6, model::Family::ModernBinary, true);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("before.accdb");
    let output = directory.path().join("after.accdb");
    fs::write(&source, &case.before).unwrap();
    let result = typed(&case, &source, &output, true);
    assert!(!result.status.success());
    assert!(!output.exists());
    same_image(&fs::read(&source).unwrap(), &case.before);

    for flags in [&["--long-value"][..], &["--index-growth"][..]] {
        let result = sql(
            "UPDATE Items SET [First]='x', [Second]=[Second] WHERE FALSE",
            &source,
            &output,
            flags,
        );
        assert!(!result.status.success());
        assert!(!output.exists());
        same_image(&fs::read(&source).unwrap(), &case.before);
    }
}
