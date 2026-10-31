//! Existing CLI transports over one independently frozen fitting Text/fixed model.
use super::{
    fixed_model::{Assignment, Cell, Column},
    joint_text_model as model,
};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn fixed_kind(column: Column, assignment: &Assignment) -> &'static str {
    let carrier = if assignment.expected != Cell::Null {
        assignment.expected
    } else {
        assignment.replacement
    };
    match carrier {
        Cell::Boolean(_) => "boolean",
        Cell::Int32(_) => "int32",
        Cell::Int64(_) => "int64",
        Cell::Currency(_) => "currency-scaled",
        Cell::Date(_) => "date-bits",
        Cell::Null => match column {
            Column::Large => "int64",
            Column::LargeInt32 | Column::IndexedNumber => "int32",
            Column::Currency => "currency-scaled",
            Column::Date | Column::Time => "date-bits",
            Column::Yes | Column::No => unreachable!("Boolean is not a nullable carrier"),
        },
    }
}

fn fixed_value(cell: Cell) -> String {
    let value = match cell {
        Cell::Null => return "null".to_owned(),
        Cell::Boolean(value) => value.to_string(),
        Cell::Int32(value) => value.to_string(),
        Cell::Int64(value) | Cell::Currency(value) => value.to_string(),
        Cell::Date(value) => format!("{value:016x}"),
    };
    format!("value:{value}")
}

fn text_value(value: Option<&str>) -> String {
    match value {
        None => "null".to_owned(),
        Some(value) => format!("value:{value}"),
    }
}

fn assignments(case: &model::Case, reverse: bool) -> Vec<[String; 6]> {
    assert!(!case.texts.is_empty());
    assert!(case.fixed.iter().all(|group| !group.updates.is_empty()));
    let mut texts = case.texts.clone();
    let mut fixed = case.fixed.clone();
    if reverse {
        texts.reverse();
        fixed.reverse();
        for group in &mut fixed {
            group.updates.reverse();
        }
    }
    let mut text_assignments = texts
        .iter()
        .map(|text| {
            [
                "Items".to_owned(),
                "Short_Text".to_owned(),
                text.row.to_string(),
                "text".to_owned(),
                text_value(text.expected),
                text_value(text.replacement),
            ]
        })
        .collect::<Vec<_>>();
    let mut fixed_assignments = fixed
        .iter()
        .flat_map(|group| {
            group.updates.iter().map(|assignment| {
                [
                    group.table.to_owned(),
                    group.name.to_owned(),
                    assignment.row.to_string(),
                    fixed_kind(group.column, assignment).to_owned(),
                    fixed_value(assignment.expected),
                    fixed_value(assignment.replacement),
                ]
            })
        })
        .collect::<Vec<_>>();
    if reverse {
        fixed_assignments.append(&mut text_assignments);
        fixed_assignments
    } else {
        text_assignments.append(&mut fixed_assignments);
        text_assignments
    }
}

fn typed(
    case: &model::Case,
    reverse: bool,
    source: &Path,
    output: &Path,
    flags: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["write", "preview-update"])
        .arg(source)
        .arg(output);
    for assignment in assignments(case, reverse) {
        command.arg("--set").args(assignment);
    }
    command.args(flags).output().unwrap()
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
        .filter(|(_, (left, right))| left != right)
        .take(8)
        .map(|(at, (left, right))| (at, *left, *right))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "first image differences {differences:?}"
    );
}

fn assert_refused(result: Output, source: &Path, output: &Path, original: &[u8]) {
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!result.stderr.is_empty());
    assert!(!output.exists());
    assert_image(&fs::read(source).unwrap(), original);
}

#[test]
fn fitting_joint_text_cli_matches_thirty_representable_images_in_both_orders() {
    let mut copies = 0;
    let mut unrepresentable = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let Some(expected) = &case.expected else {
            continue;
        };
        if case.texts.is_empty() || case.fixed.iter().any(|group| group.updates.is_empty()) {
            // --set carries actual assignments, not empty declarations. Keep
            // these three literal cases in native/Access coverage; SQL below
            // exercises genuine all-empty groups without fabricating noops.
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
            let result = typed(&case, reverse, &source, &output, &[]);
            assert!(
                result.status.success(),
                "{recipe:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            assert_image(&fs::read(&output).unwrap(), expected);
            assert_image(&fs::read(&source).unwrap(), &case.before);
            assert!(
                String::from_utf8(result.stdout)
                    .unwrap()
                    .contains(&format!("Candidate rows\t{}\n", case.changed))
            );
            copies += 1;
        }
    }
    assert_eq!((copies, unrepresentable), (30, 3));
}

#[test]
fn fitting_joint_text_sql_reads_original_text_and_admits_true_empty_groups() {
    let mut copies = 0;
    for recipe in [model::Recipe::BetaBoolean, model::Recipe::EntirelyEmpty] {
        let case = model::source(6, recipe);
        let (mut assignments, identifier, matched) = if recipe == model::Recipe::BetaBoolean {
            (
                vec![
                    "[Short_Text]='beta'".to_owned(),
                    "[Yes_Field]=(Len([Short_Text])=5)".to_owned(),
                ],
                1,
                1,
            )
        } else {
            let mut targets = vec!["[Short_Text]=[Short_Text]".to_owned()];
            targets.extend(
                case.fixed
                    .iter()
                    .map(|group| format!("[{}]=[{}]", group.name, group.name)),
            );
            (targets, -1, 0)
        };
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        fs::write(&source, &case.before).unwrap();
        for reverse in [false, true] {
            if reverse {
                assignments.reverse();
            }
            let statement = format!(
                "UPDATE [Items] SET {} WHERE [ID]={identifier}",
                assignments.join(",")
            );
            let output = directory.path().join(if reverse {
                "reverse.accdb"
            } else {
                "forward.accdb"
            });
            let result = sql(&source, &output, &statement);
            assert!(
                result.status.success(),
                "{recipe:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            assert_image(&fs::read(&output).unwrap(), case.expected.as_ref().unwrap());
            assert_image(&fs::read(&source).unwrap(), &case.before);
            assert!(String::from_utf8(result.stdout).unwrap().contains(&format!(
                "Matched rows\t{matched}\nCandidate rows\t{}\n",
                case.changed,
            )));
            copies += 1;
        }
    }
    assert_eq!(copies, 4);
}

#[test]
fn fitting_joint_text_cli_refusals_preserve_source_and_create_no_output() {
    let mut refusals = 0;
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        if case.refusal.is_none() || recipe == model::Recipe::EmptyIndexedFixed {
            continue;
        }
        assert!(case.expected.is_none());
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        fs::write(&source, &case.before).unwrap();
        for reverse in [false, true] {
            let output = directory.path().join(if reverse {
                "reverse.accdb"
            } else {
                "forward.accdb"
            });
            assert_refused(
                typed(&case, reverse, &source, &output, &[]),
                &source,
                &output,
                &case.before,
            );
            refusals += 1;
        }
    }
    assert_eq!(refusals, 16);
    let case = model::source(6, model::Recipe::EmptyIndexedFixed);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let output = directory.path().join("candidate.accdb");
    fs::write(&source, &case.before).unwrap();
    let result = sql(
        &source,
        &output,
        "UPDATE [Items] SET [Short_Text]=[Short_Text],[Number]=[Number] WHERE [ID]=-1",
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("IndexedColumn"));
    assert_refused(result, &source, &output, &case.before);
}

#[test]
fn fitting_joint_text_cli_keeps_header_five_growth_modes_and_output_conflicts_closed() {
    let mut refused_headers = 0;
    for recipe in model::RECIPES {
        let case = model::source(5, recipe);
        if case.expected.is_none()
            || case.texts.is_empty()
            || case.fixed.iter().any(|group| group.updates.is_empty())
        {
            continue;
        }
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        let output = directory.path().join("candidate.accdb");
        fs::write(&source, &case.before).unwrap();
        let result = typed(&case, false, &source, &output, &[]);
        assert!(String::from_utf8_lossy(&result.stderr).contains("unsupported"));
        assert_refused(result, &source, &output, &case.before);
        refused_headers += 1;
    }
    assert_eq!(refused_headers, 15);
    let case = model::source(6, model::Recipe::BetaBoolean);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    fs::write(&source, &case.before).unwrap();
    for flags in [
        vec!["--variable"],
        vec!["--variable", "--index-growth"],
        vec!["--long-value"],
    ] {
        let output = directory.path().join("wrong-mode.accdb");
        assert_refused(
            typed(&case, false, &source, &output, &flags),
            &source,
            &output,
            &case.before,
        );
    }
    let existing = directory.path().join("existing.accdb");
    fs::write(&existing, b"existing output").unwrap();
    for output in [&source, &existing] {
        let result = typed(&case, false, &source, output, &[]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert_image(&fs::read(&source).unwrap(), &case.before);
        assert_eq!(fs::read(&existing).unwrap(), b"existing output");
    }
}
