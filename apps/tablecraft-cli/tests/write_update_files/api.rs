//! Bounded TSV UPDATE copies retain original expected values and publication law.
use super::{cases, forwarding, large};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;

fn fields(case: &cases::Case) -> [String; 6] {
    fn wire(value: &Value) -> String {
        match value {
            Value::Null => "null".into(),
            Value::Text(text) => format!("value:{text}"),
            Value::Binary(bytes) => bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
            _ => unreachable!(),
        }
    }
    [
        "Items".into(),
        case.column.into(),
        "0".into(),
        if case.column == "Memo" {
            "text"
        } else {
            "binary"
        }
        .into(),
        wire(&case.before_value),
        wire(&case.value),
    ]
}

fn tsv(fields: &[String]) -> String {
    fields
        .iter()
        .map(|field| format!("\"{}\"", field.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join("\t")
        + "\n"
}

fn run(input: &Path, output: &Path, requests: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["write", "preview-update"])
        .arg(input)
        .arg(output)
        .arg("--set-file")
        .arg(requests)
        .args(["--long-value", "--value-bytes", "262150"])
        .output()
        .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    assert!(first.is_none(), "whole image mismatch at {first:?}");
}

#[test]
fn file_and_argv_preserve_all_qualified_images_including_large_memo_and_binary() {
    let mut selected = cases::cases();
    let position = selected
        .iter()
        .position(|case| case.column == "Ole" && case.value == Value::Binary(vec![8]))
        .unwrap();
    selected[position] = forwarding::case();
    selected.push(large::case());
    for case in selected {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("before.accdb");
        let output = directory.path().join("after.accdb");
        let requests = directory.path().join("requests.tsv");
        let cells = fields(&case);
        let contents = tsv(&cells);
        fs::write(&input, &case.before).unwrap();
        fs::write(&requests, &contents).unwrap();
        let result = run(&input, &output, &requests);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert_image(&fs::read(&output).unwrap(), &case.expected);
        assert_image(&fs::read(&input).unwrap(), &case.before);
        assert_eq!(fs::read_to_string(&requests).unwrap(), contents);

        if contents.len() < 16_384 {
            let inline = directory.path().join("inline.accdb");
            let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["write", "preview-update"])
                .arg(&input)
                .arg(&inline)
                .arg("--set")
                .args(&cells)
                .args(["--long-value", "--value-bytes", "262150"])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_image(&fs::read(&inline).unwrap(), &case.expected);
        }
        assert_eq!(run(&input, &output, &requests).status.code(), Some(2));
        assert_image(&fs::read(&output).unwrap(), &case.expected);
        assert_image(&fs::read(&input).unwrap(), &case.before);
    }
}

#[test]
fn stale_expected_and_malformed_file_never_publish_or_open_missing_source() {
    let case = large::case();
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    let output = directory.path().join("refused.accdb");
    let requests = directory.path().join("requests.tsv");
    fs::write(&input, &case.before).unwrap();
    let mut cells = fields(&case);
    cells[4] = "value:stale".into();
    fs::write(&requests, tsv(&cells)).unwrap();
    let stale = run(&input, &output, &requests);
    assert_eq!(stale.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&stale.stderr).contains("does not match its captured row"));
    assert!(stale.stdout.is_empty());
    assert!(!output.exists());
    assert_image(&fs::read(&input).unwrap(), &case.before);

    for (contents, message) in [
        ("Items\tMemo\t0\ttext\tnull\t\"unterminated", "Unterminated"),
        ("Items\tMemo\t0\ttext\tnull", "six-column"),
        ("Items\tMemo\t0\tunknown\tnull\tnull", "Unknown"),
    ] {
        fs::write(&requests, contents).unwrap();
        let missing = directory.path().join("missing.accdb");
        let result = run(&missing, &output, &requests);
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains(message));
        assert!(!output.exists());
    }
}
