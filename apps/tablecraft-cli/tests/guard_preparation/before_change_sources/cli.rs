use super::{CASES, expected, physical_roles};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const QUALIFIED_REQUIRED: bool = cfg!(any(target_os = "macos", target_os = "linux"));

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    let differences: Vec<_> = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (actual, expected))| actual != expected)
        .take(8)
        .map(|(offset, (actual, expected))| (offset, *actual, *expected))
        .collect();
    assert!(
        differences.is_empty(),
        "first byte differences: {differences:?}"
    );
}

fn invoke(
    input: &Path,
    output: &Path,
    sql: &str,
    properties: &[(&str, &str)],
    required: bool,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["query", "preview-before-change", sql])
        .arg("--database")
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--trace");
    for (kind, name) in properties {
        command.args(["--guard-property", kind, name]);
    }
    if required {
        command.arg("--durable-output");
    }
    command.output().unwrap()
}

#[test]
fn before_change_cli_default_and_required_copies_match_all_original_expectations() {
    let mut copies = 0;
    let mut rejections = 0;
    for required in [false, true] {
        if required && !QUALIFIED_REQUIRED {
            continue;
        }
        for code in [5, 6] {
            for (sql, body, score) in CASES {
                let (before, _, _) = physical_roles::source(code, body);
                let root = tempfile::tempdir().unwrap();
                let input = root.path().join("source.accdb");
                let output = root.path().join("copy.accdb");
                fs::write(&input, &before).unwrap();
                let result = invoke(&input, &output, sql, &[("3", "StoredGuard")], required);
                if score.is_none() {
                    assert!(!result.status.success());
                    assert!(result.stdout.is_empty());
                    let error = String::from_utf8_lossy(&result.stderr);
                    assert!(error.contains("Error Number 17"), "{error}");
                    assert!(!output.exists());
                    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                    rejections += 1;
                } else {
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    assert!(result.stderr.is_empty());
                    assert_image(&fs::read(&output).unwrap(), &expected(&before, score));
                    let report = String::from_utf8(result.stdout).unwrap();
                    assert!(report.contains("Matched rows\t1\n"));
                    assert!(report.contains(&format!(
                        "Changed rows\t{}\n",
                        usize::from(score != Some(20))
                    )));
                    assert!(report.contains("Selected programs\t1\n"));
                    assert!(report.contains("SetField assignments\t1\n"));
                    assert!(
                        report.contains("Original decoded XML UTF-8 bytes; not database offsets")
                    );
                    assert!(!report.contains("<Action"));
                    assert_eq!(report.contains("Durable output\t"), required);
                    copies += 1;
                }
                assert_image(&fs::read(&input).unwrap(), &before);
            }
        }
    }
    assert_eq!(copies, if QUALIFIED_REQUIRED { 12 } else { 6 });
    assert_eq!(rejections, if QUALIFIED_REQUIRED { 4 } else { 2 });
}

#[test]
fn before_change_cli_never_replaces_occupied_outputs_or_changes_sources() {
    let mut occupied = 0;
    for required in [false, true] {
        if required && !QUALIFIED_REQUIRED {
            continue;
        }
        for code in [5, 6] {
            for (sql, body, score) in CASES {
                if score.is_none() {
                    continue;
                }
                let (before, _, _) = physical_roles::source(code, body);
                let root = tempfile::tempdir().unwrap();
                let input = root.path().join("source.accdb");
                let output = root.path().join("occupied.accdb");
                fs::write(&input, &before).unwrap();
                fs::write(&output, b"existing output").unwrap();
                let result = invoke(&input, &output, sql, &[("3", "StoredGuard")], required);
                assert!(!result.status.success());
                assert!(result.stdout.is_empty());
                assert_image(&fs::read(&input).unwrap(), &before);
                assert_eq!(fs::read(&output).unwrap(), b"existing output");
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
                occupied += 1;
            }
        }
    }
    assert_eq!(occupied, if QUALIFIED_REQUIRED { 12 } else { 6 });
}

#[test]
fn before_change_cli_empty_selection_still_qualifies_original_programs_and_profile() {
    for code in [5, 6] {
        let (_, body, _) = CASES[0];
        let (before, _, _) = physical_roles::source(code, body);
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("source.accdb");
        fs::write(&input, &before).unwrap();
        for required in [false, true] {
            if required && !QUALIFIED_REQUIRED {
                continue;
            }
            let output = root.path().join(if required {
                "required.accdb"
            } else {
                "default.accdb"
            });
            let result = invoke(
                &input,
                &output,
                "UPDATE Items SET Score=99 WHERE Id=999",
                &[("3", "StoredGuard")],
                required,
            );
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_image(&fs::read(&output).unwrap(), &before);
            let report = String::from_utf8(result.stdout).unwrap();
            assert!(report.contains("Matched rows\t0\n"));
            assert!(report.contains("Changed rows\t0\n"));
            assert!(report.contains("SetField assignments\t0\n"));
        }
        for properties in [
            vec![("3", "Missing")],
            vec![("3", "StoredGuard"), ("3", "StoredGuard")],
        ] {
            let output = root.path().join("refused.accdb");
            let result = invoke(
                &input,
                &output,
                "UPDATE Items SET Score=99 WHERE Id=999",
                &properties,
                false,
            );
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!output.exists());
        }
        let output = root.path().join("ordinary-refused.accdb");
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "preview-update", CASES[0].0])
            .arg("--database")
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("ActiveContent"));
        assert!(!output.exists());
        assert_image(&fs::read(input).unwrap(), &before);
    }
}

#[path = "cli_guards.rs"]
mod guards;
