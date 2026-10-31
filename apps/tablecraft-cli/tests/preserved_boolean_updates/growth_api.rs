//! Actual SQL and typed commands reuse one frozen rich growth model.
use super::growth_model as model;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn statement(shape: model::Shape) -> &'static str {
    match shape {
        model::Shape::Mixed | model::Shape::MixedReverse => {
            "UPDATE Items SET [Short_Text]=IIf([ID]=1,'雪雪雪雪雪雪',IIf([ID]=2,NULL,'雪')) WHERE [ID]>=1 AND [ID]<=3"
        }
        model::Shape::Noops => {
            "UPDATE Items SET [Short_Text]=IIf([ID]=1,'alpha',NULL) WHERE [ID]<=2"
        }
        model::Shape::ZeroNull => "UPDATE Items SET [Short_Text]='雪雪雪雪雪雪' WHERE [ID]=2",
        _ => "UPDATE Items SET [Short_Text]='雪雪雪雪雪雪' WHERE [ID]=1",
    }
}

fn run(
    source: &Path,
    output: &Path,
    case: &model::Case,
    shape: model::Shape,
    typed: bool,
    flags: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    if typed {
        command
            .args(["write", "preview-update"])
            .arg(source)
            .arg(output);
        for request in &case.requests {
            let encode =
                |text: Option<&str>| text.map_or("null".to_owned(), |text| format!("value:{text}"));
            command.args([
                "--set".to_owned(),
                "Items".to_owned(),
                "Short_Text".to_owned(),
                request.row.to_string(),
                "text".to_owned(),
                encode(request.expected),
                encode(request.replacement),
            ]);
        }
    } else {
        command
            .args(["query", "preview-update", statement(shape), "--database"])
            .arg(source)
            .arg("--output")
            .arg(output);
    }
    command.args(flags).output().unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .take(8)
        .map(|(at, (left, right))| (at, *left, *right))
        .collect::<Vec<_>>();
    assert!(differences.is_empty(), "first differences {differences:?}");
}

#[test]
fn explicit_projected_text_growth_sql_and_typed_cli_match_eight_images_each() {
    for shape in model::SHAPES {
        let case = model::source(shape);
        for typed in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("source.accdb");
            let output = directory.path().join("candidate.accdb");
            fs::write(&source, &case.before).unwrap();
            let result = run(&source, &output, &case, shape, typed, &["--variable"]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            assert_image(&fs::read(&output).unwrap(), &case.expected);
            assert_image(&fs::read(&source).unwrap(), &case.before);
            let report = String::from_utf8(result.stdout).unwrap();
            if !typed {
                assert!(report.contains(&format!("Matched rows\t{}\n", case.requests.len())));
            }
            assert!(report.contains(&format!("Candidate rows\t{}\n", case.changed)));
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);

            fs::write(&output, b"occupied").unwrap();
            let occupied = run(&source, &output, &case, shape, typed, &["--variable"]);
            assert!(!occupied.status.success());
            assert!(occupied.stdout.is_empty());
            assert_eq!(fs::read(&output).unwrap(), b"occupied");
            assert_image(&fs::read(&source).unwrap(), &case.before);
        }
    }
}

#[test]
fn projected_growth_cli_keeps_fitting_default_and_other_modes_closed() {
    let mut case = model::source(model::Shape::OriginalPage);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    fs::write(&source, &case.before).unwrap();
    for typed in [false, true] {
        for (index, flags) in [
            vec![],
            vec!["--variable", "--index-growth"],
            vec!["--long-value"],
            vec!["--index-growth"],
            vec!["--cascade-updates"],
        ]
        .into_iter()
        .enumerate()
        {
            let output = directory
                .path()
                .join(format!("refused-{typed}-{index}.accdb"));
            let result = run(
                &source,
                &output,
                &case,
                model::Shape::OriginalPage,
                typed,
                &flags,
            );
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!output.exists());
        }
    }
    let output = directory.path().join("larger-refused.accdb");
    let result = run(
        &source,
        &output,
        &case,
        model::Shape::OriginalPage,
        false,
        &["--image-bytes", "67108864"],
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!output.exists());

    case.requests[0].expected = Some("stale");
    let output = directory.path().join("stale-refused.accdb");
    let result = run(
        &source,
        &output,
        &case,
        model::Shape::OriginalPage,
        true,
        &["--variable"],
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!output.exists());
    assert_image(&fs::read(&source).unwrap(), &case.before);

    case.requests[0].expected = Some("alpha");
    case.before[20] = 5;
    fs::write(&source, &case.before).unwrap();
    for typed in [false, true] {
        let output = directory.path().join(format!("header5-{typed}.accdb"));
        let result = run(
            &source,
            &output,
            &case,
            model::Shape::OriginalPage,
            typed,
            &["--variable"],
        );
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
    }
    assert_image(&fs::read(&source).unwrap(), &case.before);
}
