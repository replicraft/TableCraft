//! Explicit fixed-purpose capture reaches one existing SQL finish and publisher.
use super::corpus::{corrected, model};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const WORK_BYTES: &str = "2147483648";
const WORK_STEPS: &str = "1000000000";

fn command(sql: &str, input: &Path, output: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", sql, "--database"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(["--work-bytes", WORK_BYTES, "--work-steps", WORK_STEPS])
        .args(flags)
        .output()
        .unwrap()
}

fn sql(shape: model::Shape) -> &'static str {
    if shape.preserved() {
        "UPDATE Items SET [Yes_Field]=TRUE WHERE [ID]=1"
    } else {
        "UPDATE Items SET [Score]=11 WHERE [Id]=1"
    }
}

fn same_image(actual: &[u8], expected: &[u8]) {
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    assert!(
        actual == expected,
        "whole image lengths {}/{}, first {first:?}",
        actual.len(),
        expected.len()
    );
}

#[test]
fn explicit_fixed_image_limit_publishes_all10_native_qualified_complete_images() {
    let mut images = 0;
    for code in [5, 6] {
        for shape in model::Shape::ALL {
            if code == 5 && shape.preserved() {
                continue;
            }
            let case = corrected::case(code, shape);
            let directory = tempfile::tempdir().unwrap();
            let input = directory.path().join("before.accdb");
            let output = directory.path().join("after.accdb");
            fs::write(&input, &case.before).unwrap();
            let ceiling = case.before.len().to_string();

            let result = command(sql(shape), &input, &output, &["--image-bytes", &ceiling]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            same_image(&fs::read(&output).unwrap(), &case.expected);
            same_image(&fs::read(&input).unwrap(), &case.before);

            let occupied = command(sql(shape), &input, &output, &["--image-bytes", &ceiling]);
            assert_eq!(occupied.status.code(), Some(2));
            assert!(occupied.stdout.is_empty());
            same_image(&fs::read(&output).unwrap(), &case.expected);
            same_image(&fs::read(&input).unwrap(), &case.before);
            images += 1;
        }
    }
    assert_eq!(images, 10);
}

#[test]
fn default_ceiling_one_below_and_work_budget_do_not_expand_with_the_flag() {
    let case = corrected::case(6, model::Shape::ScalarLargeHigh);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    fs::write(&input, &case.before).unwrap();
    let one_below = (case.before.len() - 1).to_string();
    for (ordinal, flags) in [&[][..], &["--image-bytes", one_below.as_str()][..]]
        .into_iter()
        .enumerate()
    {
        let output = directory.path().join(format!("refused{ordinal}.accdb"));
        let result = command(sql(case.shape), &input, &output, flags);
        assert_eq!(result.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("database inspection structural limit exceeded")
        );
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        same_image(&fs::read(&input).unwrap(), &case.before);
    }

    let output = directory.path().join("budget.accdb");
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", sql(case.shape), "--database"])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .args(["--image-bytes", "83886080", "--work-bytes", "1"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(!output.exists());
    same_image(&fs::read(&input).unwrap(), &case.before);
}

#[test]
fn incompatible_modes_and_invalid_limits_refuse_before_opening_a_source() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("missing.accdb");
    let output = directory.path().join("refused.accdb");
    for flag in [
        "--variable",
        "--long-value",
        "--index-growth",
        "--cascade-updates",
    ] {
        let result = command(
            "UPDATE Items SET [Score]=11 WHERE [Id]=1",
            &input,
            &output,
            &["--image-bytes", "83886080", flag],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("cannot be used with"));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
    }
    for invalid in ["0", "83886081", "not-a-number", "18446744073709551616"] {
        let result = command(
            "UPDATE Items SET [Score]=11 WHERE [Id]=1",
            &input,
            &output,
            &["--image-bytes", invalid],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("invalid value"));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
    }
}

#[test]
fn header5_rich_profiles_stay_refused_under_the_explicit_ceiling() {
    for shape in [model::Shape::PreservedDefault, model::Shape::PreservedHigh] {
        let case = corrected::case(5, shape);
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("before.accdb");
        let output = directory.path().join("refused.accdb");
        fs::write(&input, &case.before).unwrap();

        let result = command(sql(shape), &input, &output, &["--image-bytes", "83886080"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        same_image(&fs::read(&input).unwrap(), &case.before);
    }
}

#[test]
fn fixed_purpose_does_not_retry_a_memo_target_even_with_zero_matches() {
    let before = super::lval_source::source(6, 1);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    let baseline = directory.path().join("default.accdb");
    let refused = directory.path().join("refused.accdb");
    fs::write(&input, &before).unwrap();
    let sql = "UPDATE Items SET [Memo]='A' WHERE FALSE";

    let default = command(sql, &input, &baseline, &[]);
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    same_image(&fs::read(&baseline).unwrap(), &before);

    let fixed = command(sql, &input, &refused, &["--image-bytes", "83886080"]);
    assert_eq!(fixed.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&fixed.stderr).contains("native assignment carrier is unsupported")
    );
    assert!(fixed.stdout.is_empty());
    assert!(!refused.exists());
    same_image(&fs::read(&input).unwrap(), &before);
}
