//! Actual typed commands and service calls share one original batch finish.
use super::super::{budget, fs};
use super::model::{self, Case, Kind};
use std::{
    io::Cursor,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_access::engineering::{self, EngineeringRequest};
use tablecraft_ace::{IncrementingAutoNumber, InsertFieldValue, NativeImage, OmittedRowInsert};
use tablecraft_core::values::{ExtendedDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_model::CatalogScope;

const FIXED_CLOCK: &str = "--default-clock=2000-02-29T12:00:00.7654321";

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

fn command(source: &Path, output: &Path, requests: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["write", "preview-insert"])
        .arg(source)
        .arg(output)
        .arg("Items")
        .arg(requests)
        .args(flags)
        .output()
        .unwrap()
}

#[test]
fn typed_batch_omissions_publish24_complete_images_under_original_counter_policies() {
    for code in [5, 6] {
        for generated in [false, true] {
            for kind in [Kind::Integer, Kind::Clock] {
                for count in [2, 3] {
                    let case = model::case(code, generated, kind, count);
                    for explicit in if generated {
                        &[false, true][..]
                    } else {
                        &[false][..]
                    } {
                        let directory = tempfile::tempdir().unwrap();
                        let source = directory.path().join("before.accdb");
                        let output = directory.path().join("after.accdb");
                        let requests = directory.path().join("rows.tsv");
                        fs::write(&source, &case.before).unwrap();
                        fs::write(&requests, &case.requests).unwrap();
                        let mut flags = Vec::new();
                        if generated {
                            if *explicit {
                                flags.extend(["--incrementing-last", "30"]);
                            } else {
                                flags.push("--incrementing");
                            }
                        }
                        if matches!(kind, Kind::Clock) {
                            flags.push(FIXED_CLOCK);
                        }
                        let result = command(&source, &output, &requests, &flags);
                        assert!(
                            result.status.success(),
                            "{}",
                            String::from_utf8_lossy(&result.stderr)
                        );
                        same_image(&fs::read(&output).unwrap(), &case.expected);
                        same_image(&fs::read(&source).unwrap(), &case.before);
                        let report = String::from_utf8(result.stdout).unwrap();
                        assert!(report.contains(&format!("Candidate rows\t{count}\n")));
                        if generated {
                            assert!(report.contains("Generated column\t1\n"));
                            for (ordinal, row) in case.added.iter().enumerate() {
                                assert!(report.contains(&format!(
                                    "Generated value[{ordinal}]\t{}\n",
                                    row.score
                                )));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn original_requests(case: &Case) -> Vec<OmittedRowInsert> {
    case.added
        .iter()
        .enumerate()
        .map(|(ordinal, row)| {
            let mut values = vec![InsertFieldValue::Supplied(Value::Int32(row.id))];
            if !case.generated {
                values.push(InsertFieldValue::Supplied(Value::Int32(row.score)));
            }
            values.push(match (case.kind, ordinal) {
                (_, 1) => InsertFieldValue::Supplied(Value::Null),
                (Kind::Integer, 2) => InsertFieldValue::Supplied(Value::Int32(77)),
                _ => InsertFieldValue::Omitted,
            });
            OmittedRowInsert::new(values)
        })
        .collect()
}

struct CountClock {
    reads: AtomicUsize,
    reading: Option<ClockReading>,
}

impl Clock for CountClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.reading.ok_or(EvalError::ClockUnavailable)
    }
}

fn fixed_reading() -> ClockReading {
    ClockReading::new(ExtendedDateTime::new(2000, 2, 29, 12, 0, 0, 7_654_321).unwrap()).unwrap()
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn original_batch_presence_reads_one_clock_only_when_a_needed_default_requires_it() {
    for code in [5, 6] {
        for generated in [false, true] {
            for kind in [Kind::Integer, Kind::Clock] {
                for count in [2, 3] {
                    let case = model::case(code, generated, kind, count);
                    let requests = original_requests(&case);
                    let clock = CountClock {
                        reads: AtomicUsize::new(0),
                        reading: matches!(kind, Kind::Clock).then(fixed_reading),
                    };
                    let request = if generated {
                        EngineeringRequest::IncrementingInsertWithClockFromSource {
                            table: "Items",
                            rows: &requests,
                            clock: &clock,
                        }
                    } else {
                        EngineeringRequest::InsertWithClock {
                            table: "Items",
                            rows: &requests,
                            clock: &clock,
                        }
                    };
                    let copy = engineering::preview(
                        capture(&case.before),
                        request,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    same_image(copy.candidate_bytes(), &case.expected);
                    assert_eq!(
                        clock.reads.load(Ordering::Relaxed),
                        usize::from(matches!(kind, Kind::Clock))
                    );
                }
            }
        }
    }
    let case = model::case(6, true, Kind::Clock, 3);
    let requests = original_requests(&case);
    let clock = CountClock {
        reads: AtomicUsize::new(0),
        reading: None,
    };
    assert!(
        engineering::preview(
            capture(&case.before),
            EngineeringRequest::IncrementingInsertWithClock {
                table: "Items",
                rows: &requests,
                policy: IncrementingAutoNumber::new(29),
                clock: &clock,
            },
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
    assert_eq!(clock.reads.load(Ordering::Relaxed), 0);
}

#[test]
fn fully_supplied_rows_keep_manual_paths_even_with_an_unused_clock_option() {
    for generated in [false, true] {
        for kind in [Kind::Integer, Kind::Clock] {
            let case = model::case(6, generated, kind, 3);
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("before.accdb");
            let output = directory.path().join("after.accdb");
            let requests = directory.path().join("complete.tsv");
            fs::write(&source, &case.before).unwrap();
            let mut text = String::new();
            for row in &case.added {
                text.push_str(&format!("int32\t{}\t", row.id));
                if !generated {
                    text.push_str(&format!("int32\t{}\t", row.score));
                }
                text.push_str(&match (kind, row.other) {
                    (_, None) => "null\t\n".into(),
                    (Kind::Integer, Some(value)) => format!("int32\t{value}\n"),
                    (Kind::Clock, Some(bits)) => format!("date-bits\t{bits:016x}\n"),
                });
            }
            fs::write(&requests, text).unwrap();
            let flags = if generated {
                &["--incrementing", "--default-clock"][..]
            } else {
                &["--default-clock"][..]
            };
            let result = command(&source, &output, &requests, flags);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            same_image(&fs::read(&output).unwrap(), &case.expected);
            same_image(&fs::read(&source).unwrap(), &case.before);
        }
    }
}

#[test]
fn late_conflicts_invalid_presence_and_missing_clock_publish_no_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("before.accdb");
    let output = directory.path().join("after.accdb");
    let requests = directory.path().join("rows.tsv");
    let case = model::case(6, false, Kind::Integer, 3);
    fs::write(&source, &case.before).unwrap();
    let text = String::from_utf8(case.requests.clone())
        .unwrap()
        .replace("int32\t60\t", "int32\t40\t");
    assert_ne!(text.as_bytes(), case.requests);
    fs::write(&requests, text).unwrap();
    let result = command(&source, &output, &requests, &[]);
    assert!(!result.status.success());
    assert!(!output.exists());
    same_image(&fs::read(&source).unwrap(), &case.before);

    let clock_case = model::case(6, false, Kind::Clock, 3);
    fs::write(&source, &clock_case.before).unwrap();
    fs::write(&requests, &clock_case.requests).unwrap();
    let result = command(&source, &output, &requests, &[]);
    assert!(!result.status.success());
    assert!(!output.exists());
    same_image(&fs::read(&source).unwrap(), &clock_case.before);

    fs::write(&requests, b"int32\t4\tomitted\tnot-empty\n").unwrap();
    let result = command(
        &directory.path().join("missing.accdb"),
        &output,
        &requests,
        &[],
    );
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("omitted KIND requires empty DATA"));
    assert!(!output.exists());
}
