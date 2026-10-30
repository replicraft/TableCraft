use super::{
    guard_sources::{cases, model},
    inspect, selector_sources,
};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    process::Output,
    sync::atomic::{AtomicU64, Ordering},
};
use tablecraft_access::engineering::guards::{
    self, GuardBackendError, GuardPreparationSummary, GuardSqlError,
};
use tablecraft_ace::{
    BeforeEvent, BeforeEventGuardError, BeforeEventGuardSelector, BeforeGuardTraceKind,
    BeforeGuardUnsupported, NativeImage,
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{DeleteError, InsertError, UpdateError};
use tablecraft_sql::{ParseLimits, parse_query};

#[path = "../support/mod.rs"]
mod cli_support;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn sql(case: &cases::Case) -> String {
    match case.intent {
        cases::Intent::Update { row, value, .. } => format!(
            "UPDATE Items SET Score={value} WHERE Id={}",
            model::ORIGINAL_ROWS[row][0]
        ),
        cases::Intent::Insert { id, score } => {
            format!("INSERT INTO Items (Id,Score) VALUES({id},{score})")
        }
        cases::Intent::Delete { row } => format!(
            "DELETE FROM Items WHERE Id={}",
            model::ORIGINAL_ROWS[row][0]
        ),
        cases::Intent::UpdateTwo => "UPDATE Items SET Score=8*Id+3 WHERE Id<=2".into(),
    }
}

fn trace(report: &GuardPreparationSummary) -> Vec<String> {
    report
        .trace()
        .iter()
        .map(|step| match step.kind() {
            BeforeGuardTraceKind::CompiledAll => "CompiledAll".into(),
            BeforeGuardTraceKind::Begin if report.event() == BeforeEvent::Insert => {
                format!("BeginInsert:{}", step.row().unwrap())
            }
            BeforeGuardTraceKind::Begin => format!("Begin:{}", step.row().unwrap()),
            BeforeGuardTraceKind::Condition(value) => format!("If:{value}"),
            BeforeGuardTraceKind::End => format!("End:{}", step.row().unwrap()),
            BeforeGuardTraceKind::Raised(number) => format!("Raise:{number}"),
            BeforeGuardTraceKind::Passed => "Passed".into(),
            BeforeGuardTraceKind::Rejected => "Rejected".into(),
        })
        .collect()
}

fn native(error: &GuardSqlError) -> Option<&BeforeEventGuardError> {
    match error {
        GuardSqlError::Backend(GuardBackendError::Guard(error))
        | GuardSqlError::Update(UpdateError::Backend(GuardBackendError::Guard(error)))
        | GuardSqlError::Delete(DeleteError::Backend(GuardBackendError::Guard(error)))
        | GuardSqlError::Insert(InsertError::Backend(GuardBackendError::Guard(error))) => {
            Some(error)
        }
        _ => None,
    }
}

#[test]
fn guard_host_access_matches_twenty_completed_reports_and_twenty_typed_failures() {
    let mut completed = 0;
    let mut failed = 0;
    for case in cases::CASES {
        for code in [5, 6] {
            let (bytes, _, _) = model::source(code, case.event, case.body);
            let image = NativeImage::capture(
                &mut Cursor::new(&bytes),
                CatalogScope::new(7),
                &mut inspect::budget(),
                &|| false,
            )
            .unwrap();
            let text = sql(case);
            let query = parse_query(&text, ParseLimits::default()).unwrap();
            let result = guards::prepare(
                image,
                query.query(),
                &[BeforeEventGuardSelector::new(3, "StoredGuard")],
                &mut inspect::budget(),
                None,
                &|| false,
            );
            match case.outcome {
                cases::Outcome::Pass | cases::Outcome::Raised { .. } => {
                    let report = result
                        .unwrap_or_else(|error| panic!("{} / header {code}: {error}", case.name));
                    assert_eq!(trace(&report), case.trace, "{} / header {code}", case.name);
                    assert_eq!(report.properties().len(), 1);
                    assert_eq!(
                        (
                            report.properties()[0].group(),
                            report.properties()[0].name()
                        ),
                        (3, "StoredGuard")
                    );
                    match case.outcome {
                        cases::Outcome::Pass => assert!(report.rejection().is_none()),
                        cases::Outcome::Raised {
                            number,
                            description,
                        } => {
                            assert_eq!(report.rejection().unwrap().number(), number);
                            assert_eq!(report.rejection().unwrap().description(), description);
                        }
                        _ => unreachable!(),
                    }
                    completed += 1;
                }
                _ => {
                    let error = result
                        .err()
                        .unwrap_or_else(|| panic!("{} unexpectedly completed", case.name));
                    let expected = match case.outcome {
                        cases::Outcome::UnsupportedContext => {
                            BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::Context)
                        }
                        cases::Outcome::UnsupportedCondition => {
                            BeforeEventGuardError::ConditionType
                        }
                        cases::Outcome::UnsupportedStructure => {
                            BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::Structure)
                        }
                        cases::Outcome::ExpressionError => BeforeEventGuardError::Expression,
                        cases::Outcome::ArgumentType => BeforeEventGuardError::ArgumentType,
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        native(&error),
                        Some(&expected),
                        "{} / header {code}: {error}",
                        case.name
                    );
                    failed += 1;
                }
            }
        }
    }
    assert_eq!((completed, failed), (20, 20));
}

struct Database {
    path: PathBuf,
    directory: PathBuf,
    bytes: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "tablecraft-guard-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("source.accdb");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        Self {
            path,
            directory,
            bytes,
        }
    }

    fn run(&self, sql: &str, properties: &[(&str, &str)]) -> Output {
        let mut args = vec![
            "query",
            "prepare-guards",
            sql,
            "--database",
            self.path.to_str().unwrap(),
            "--trace",
        ];
        for (kind, name) in properties {
            args.extend(["--guard-property", kind, name]);
        }
        cli_support::run(&args)
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        assert!(
            fs::read(&self.path).unwrap() == self.bytes,
            "guard command changed its source"
        );
        assert_eq!(
            fs::read_dir(&self.directory).unwrap().count(),
            1,
            "guard command created another file"
        );
        fs::remove_file(&self.path).unwrap();
        fs::remove_dir(&self.directory).unwrap();
    }
}

#[test]
fn guard_host_cli_matches_twenty_completed_reports_and_twenty_no_stdout_failures() {
    let mut completed = 0;
    let mut failed = 0;
    for case in cases::CASES {
        for code in [5, 6] {
            let (bytes, _, _) = model::source(code, case.event, case.body);
            let database = Database::new(bytes);
            let output = database.run(&sql(case), &[("3", "StoredGuard")]);
            match case.outcome {
                cases::Outcome::Pass | cases::Outcome::Raised { .. } => {
                    assert!(
                        output.status.success(),
                        "{} / header {code}: {}",
                        case.name,
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert!(output.stderr.is_empty());
                    let text = String::from_utf8(output.stdout).unwrap();
                    assert!(text.contains("Selected property"));
                    assert!(text.contains("\"StoredGuard\""));
                    assert!(text.contains("Candidate\tNot created\n"));
                    assert!(
                        text.contains(
                            "Normal save / macro absence\tNot authorized / not certified\n"
                        )
                    );
                    match case.outcome {
                        cases::Outcome::Pass => {
                            assert!(text.contains("Guard decision\tPassed subset\n"))
                        }
                        cases::Outcome::Raised { number, .. } => {
                            assert!(text.contains("Guard decision\tRejected\n"));
                            assert!(text.contains(&format!("Error Number\t{number}\n")));
                        }
                        _ => unreachable!(),
                    }
                    completed += 1;
                }
                _ => {
                    assert!(!output.status.success(), "{} / header {code}", case.name);
                    assert!(output.stdout.is_empty());
                    assert!(!output.stderr.is_empty());
                    failed += 1;
                }
            }
        }
    }
    assert_eq!((completed, failed), (20, 20));
}

#[test]
fn guard_host_cli_names_caller_scope_order_and_never_runs_unselected_xml() {
    for code in [5, 6] {
        let source = selector_sources::model::source(code, selector_sources::model::Variant::Valid);
        let database = Database::new(source.bytes);
        for (properties, number, first) in [
            ([("3", "GuardA"), ("3", "GuardB")], 71, "GuardA"),
            ([("3", "GuardB"), ("3", "GuardA")], 73, "GuardB"),
        ] {
            let output = database.run("UPDATE Items SET Score=11 WHERE Id=1", &properties);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains(&format!("Error Number\t{number}\n")));
            assert!(text.contains(&format!("0\t3\t\"{first}\"\n")));
        }
        for variant in [
            selector_sources::model::Variant::SecondOpaque,
            selector_sources::model::Variant::SecondMalformed,
        ] {
            let source = selector_sources::model::source(code, variant);
            let database = Database::new(source.bytes);
            let output = database.run("UPDATE Items SET Score=11 WHERE Id=1", &[("3", "GuardA")]);
            assert!(
                output.status.success(),
                "{variant:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("Error Number\t71\n")
            );
            let output = database.run(
                "UPDATE Items SET Score=11 WHERE Id=1",
                &[("3", "GuardA"), ("3", "GuardB")],
            );
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
    }
}
