//! Actual SQL publication uses the same original-source LVAL request.
use super::{cases, forwarding};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;

fn run(sql: &str, input: &Path, output: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", sql, "--database"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(flags)
        .output()
        .unwrap()
}

#[test]
fn default_and_explicit_lval_publish_exact_text_null_and_empty_copies() {
    for case in qualified_cases() {
        let literal = match &case.value {
            Value::Text(value) => format!("'{}'", value.replace('\'', "''")),
            Value::Null => "NULL".to_owned(),
            Value::Binary(_) => continue,
            _ => unreachable!(),
        };
        let sql = format!("UPDATE Items SET [{}]={literal} WHERE [Id]=1", case.column);
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("before.accdb");
        fs::write(&input, &case.before).unwrap();
        for (ordinal, flags) in [&[][..], &["--long-value"][..]].into_iter().enumerate() {
            let output = directory.path().join(format!("after{ordinal}.accdb"));
            let result = run(&sql, &input, &output, flags);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            assert_image(&fs::read(&output).unwrap(), &case.expected);
            assert_image(&fs::read(&input).unwrap(), &case.before);

            let occupied = run(&sql, &input, &output, flags);
            assert_eq!(occupied.status.code(), Some(2));
            assert_image(&fs::read(&output).unwrap(), &case.expected);
            assert_image(&fs::read(&input).unwrap(), &case.before);
        }
    }
}

#[test]
fn explicit_variable_and_joint_lval_refusals_never_publish_even_without_matches() {
    let case = qualified_cases().remove(0);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    fs::write(&input, &case.before).unwrap();
    for (ordinal, (sql, flags)) in [
        (
            "UPDATE Items SET [Memo]='x' WHERE FALSE",
            &["--variable"][..],
        ),
        ("UPDATE Items SET [Memo]='x',[Id]=1 WHERE FALSE", &[][..]),
        ("UPDATE Items SET [Memo]='x',[Id]=1 WHERE [Id]=1", &[][..]),
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused{ordinal}.accdb"));
        let result = run(sql, &input, &output, flags);
        assert_eq!(result.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("native write profile is unsupported (Column)")
        );
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        assert_image(&fs::read(&input).unwrap(), &case.before);
    }
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    let count = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
    assert!(
        first.is_none(),
        "complete image mismatch: {count} bytes; first offset {first:?}"
    );
}

// The predecessor compacting expectation remains in its original source owner.
// Only the independently qualified stable-root case is used for API replay.
fn qualified_cases() -> Vec<cases::Case> {
    let mut selected = cases::cases();
    let position = selected
        .iter()
        .position(|case| {
            case.column == "Ole"
                && case.before_value == Value::Binary(vec![7])
                && case.value == Value::Binary(vec![8])
        })
        .expect("the original external OLE replacement case");
    selected[position] = forwarding::case();
    selected
}
