//! Real default SQL command over the one frozen Short Text source owner.
use super::short_text_model as model;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn statement(case: &model::Case) -> String {
    let replacement = match case.replacement {
        Some(text) => format!("'{}'", text.replace('\'', "''")),
        None => "NULL".to_owned(),
    };
    format!(
        "UPDATE Items SET [Short_Text]={replacement} WHERE [ID]={}",
        case.framing.row + 1
    )
}

fn invoke(source: &Path, output: &Path, sql: &str, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", sql, "--database"])
        .arg(source)
        .arg("--output")
        .arg(output)
        .args(flags)
        .output()
        .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (actual, expected))| actual != expected)
        .take(8)
        .map(|(at, (actual, expected))| (at, *actual, *expected))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "First image differences: {differences:?}"
    );
}

#[test]
fn default_projected_short_text_cli_matches_eight_complete_images_and_refuses_overflow() {
    let mut copies = 0;
    let mut overflows = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        assert!(case.admitted_source);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        let output = directory.path().join("copy.accdb");
        fs::write(&source, &case.before).unwrap();
        let result = invoke(&source, &output, &statement(&case), &[]);
        if let Some(expected) = &case.expected {
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            let actual = fs::read(&output).unwrap();
            assert_image(&actual, expected);
            let payload = case.payload.unwrap();
            let start = case.framing.absolute + case.framing.offsets[case.framing.variable];
            assert_eq!(&actual[start..start + payload.len()], payload);
            let report = String::from_utf8(result.stdout).unwrap();
            assert!(report.contains("Matched rows\t1\n"));
            let changed = usize::from(case.original != case.replacement);
            assert!(report.contains(&format!("Candidate rows\t{changed}\n")));
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
            copies += 1;
        } else {
            assert_eq!(recipe, model::Recipe::Overflow);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!output.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            overflows += 1;
        }
        assert_image(&fs::read(&source).unwrap(), &case.before);
    }
    assert_eq!((copies, overflows), (8, 1));
}

#[test]
fn projected_short_text_cli_preserves_explicit_modes_and_no_clobber() {
    let case = model::source(6, model::Recipe::ShorterNarrow);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    fs::write(&source, &case.before).unwrap();
    for (position, flags) in [
        vec!["--variable"],
        vec!["--variable", "--index-growth"],
        vec!["--long-value"],
        vec!["--index-growth"],
        vec!["--cascade-updates"],
        vec!["--image-bytes", "67108864"],
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused-{position}.accdb"));
        let result = invoke(&source, &output, &statement(&case), &flags);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("unsupported"));
        assert!(!output.exists());
    }
    let occupied = directory.path().join("occupied.accdb");
    fs::write(&occupied, b"existing output").unwrap();
    let result = invoke(&source, &occupied, &statement(&case), &[]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&occupied).unwrap(), b"existing output");
    assert_image(&fs::read(&source).unwrap(), &case.before);

    let other = model::source(5, model::Recipe::ShorterNarrow);
    assert!(!other.admitted_source);
    let header_five = directory.path().join("header5.accdb");
    let output = directory.path().join("header5-refused.accdb");
    fs::write(&header_five, &other.before).unwrap();
    let result = invoke(&header_five, &output, &statement(&other), &[]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("unsupported"));
    assert!(!output.exists());
    assert_image(&fs::read(&header_five).unwrap(), &other.before);
}

#[test]
fn typed_default_projected_short_text_preserves_original_expectations_and_eight_images() {
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        let output = directory.path().join("copy.accdb");
        fs::write(&source, &case.before).unwrap();
        let ordinal = case.framing.row.to_string();
        let encode = |text: Option<&str>| match text {
            Some(text) => format!("value:{text}"),
            None => "null".to_owned(),
        };
        let expected = encode(case.original);
        let replacement = encode(case.replacement);
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["write", "preview-update"])
            .arg(&source)
            .arg(&output)
            .args([
                "--set",
                "Items",
                "Short_Text",
                &ordinal,
                "text",
                &expected,
                &replacement,
            ])
            .output()
            .unwrap();
        if let Some(expected) = &case.expected {
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_image(&fs::read(&output).unwrap(), expected);
        } else {
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!output.exists());
        }
        assert_image(&fs::read(&source).unwrap(), &case.before);
    }
}
