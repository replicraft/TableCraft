//! Named bulk intent finishes once through the already qualified native services.
use super::super::{budget, model::Kind};
use super::model::{self, Case};
use std::{
    cell::Cell,
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_access::{
    NativeEditError,
    engineering::{self, EngineeringCopy, EngineeringError, EngineeringRequest, NamedInsertMode},
};
use tablecraft_ace::{
    EditError, IncrementingAutoNumber, InsertFieldValue, InspectError, NativeImage,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExtendedDateTime, LegacyDateTime, Value},
};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_model::CatalogScope;

const CLOCK_FLAG: &str = "--default-clock=2000-02-29T12:00:00.7654321";

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for code in [5, 6] {
        for generated in [false, true] {
            for kind in [Kind::Integer, Kind::Clock] {
                for count in [2, 3] {
                    cases.push(model::reordered(code, generated, kind, count));
                }
                cases.push(model::missing_other(code, generated, kind));
            }
            cases.push(model::missing_nullable_id(code, generated));
        }
    }

    cases
}

fn original_rows(case: &Case) -> Vec<Vec<InsertFieldValue>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(case.requests.as_slice())
        .records()
        .map(|record| {
            let record = record.unwrap();
            (0..record.len())
                .step_by(2)
                .map(|position| match &record[position] {
                    "omitted" => {
                        assert_eq!(&record[position + 1], "");
                        InsertFieldValue::Omitted
                    }
                    "null" => {
                        assert_eq!(&record[position + 1], "");
                        InsertFieldValue::Supplied(Value::Null)
                    }
                    "int32" => InsertFieldValue::Supplied(Value::Int32(
                        record[position + 1].parse().unwrap(),
                    )),
                    _ => panic!("literal source request kind"),
                })
                .collect()
        })
        .collect()
}

fn direct(
    before: &[u8],
    request: EngineeringRequest<'_>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, EngineeringError> {
    let image = NativeImage::capture(
        &mut Cursor::new(before),
        CatalogScope::new(1),
        work,
        cancelled,
    )
    .map_err(|error| EngineeringError::Engine(error.into()))?;
    engineering::preview(image, request, work, cancelled)
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

struct CountClock(AtomicUsize);

impl Clock for CountClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ClockReading::new(ExtendedDateTime::new(2000, 2, 29, 12, 0, 0, 7_654_321).unwrap())
    }
}

#[test]
fn direct_named_batches_match_all28_prefrozen_images_and_read_one_needed_clock() {
    for case in cases() {
        let fields = case
            .fields
            .iter()
            .map(|field| (*field).to_owned())
            .collect::<Vec<_>>();
        let clock = CountClock(AtomicUsize::new(0));
        let request = EngineeringRequest::NamedInsert {
            table: "Items",
            fields: &fields,
            rows: original_rows(&case),
            mode: if case.generated {
                NamedInsertMode::Incrementing
            } else {
                NamedInsertMode::Ordinary
            },
            clock: Some(&clock),
        };
        let copy = direct(&case.before, request, &mut budget(), &|| false).unwrap();
        same_image(copy.before_bytes(), &case.before);
        same_image(copy.candidate_bytes(), &case.expected);
        assert_eq!(
            clock.0.load(Ordering::Relaxed),
            usize::from(matches!(case.kind, Kind::Clock))
        );
    }
}

fn command(
    input: &Path,
    output: &Path,
    requests: &Path,
    fields: &[&str],
    flags: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["write", "preview-insert"])
        .arg(input)
        .arg(output)
        .arg("Items")
        .arg(requests);
    for field in fields {
        command.args(["--field", field]);
    }

    command.args(flags).output().unwrap()
}

#[test]
fn named_cli_batches_publish42_prefrozen_images_under_both_counter_policies() {
    let mut images = 0;
    for case in cases() {
        for explicit in if case.generated {
            &[false, true][..]
        } else {
            &[false][..]
        } {
            let directory = tempfile::tempdir().unwrap();
            let input = directory.path().join("before.accdb");
            let output = directory.path().join("after.accdb");
            let requests = directory.path().join("requests.tsv");
            fs::write(&input, &case.before).unwrap();
            fs::write(&requests, &case.requests).unwrap();
            let mut flags = Vec::new();
            if case.generated {
                if *explicit {
                    flags.extend(["--incrementing-last", "30"]);
                } else {
                    flags.push("--incrementing");
                }
            }
            if matches!(case.kind, Kind::Clock) {
                flags.push(CLOCK_FLAG);
            }
            let result = command(&input, &output, &requests, &case.fields, &flags);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            same_image(&fs::read(&output).unwrap(), &case.expected);
            same_image(&fs::read(&input).unwrap(), &case.before);
            assert_eq!(fs::read(&requests).unwrap(), case.requests);
            let occupied = command(&input, &output, &requests, &case.fields, &flags);
            assert_eq!(occupied.status.code(), Some(2));
            same_image(&fs::read(&output).unwrap(), &case.expected);
            same_image(&fs::read(&input).unwrap(), &case.before);
            images += 1;
        }
    }
    assert_eq!(images, 42);
}

#[test]
fn bound_names_shapes_and_generation_fail_before_clock_or_candidate() {
    let case = model::missing_other(6, true, Kind::Clock);
    let clock = CountClock(AtomicUsize::new(0));
    for (fields, rows) in [
        (
            vec!["Id", "id"],
            vec![vec![InsertFieldValue::Omitted, InsertFieldValue::Omitted]],
        ),
        (vec!["Missing"], vec![vec![InsertFieldValue::Omitted]]),
        (
            vec!["Score"],
            vec![vec![InsertFieldValue::Supplied(Value::Int32(31))]],
        ),
        (vec!["Id"], vec![vec![]]),
        (
            vec!["Id"],
            vec![vec![InsertFieldValue::Supplied(Value::Text(
                "wrong".into(),
            ))]],
        ),
    ] {
        let fields = fields.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let result = direct(
            &case.before,
            EngineeringRequest::NamedInsert {
                table: "Items",
                fields: &fields,
                rows,
                mode: NamedInsertMode::Incrementing,
                clock: Some(&clock),
            },
            &mut budget(),
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EngineeringError::Binding(NativeEditError::Unsupported))
        ));
        assert_eq!(clock.0.load(Ordering::Relaxed), 0);
    }
    let fields = vec!["Id".to_owned()];
    let result = direct(
        &case.before,
        EngineeringRequest::NamedInsert {
            table: "Items",
            fields: &fields,
            rows: original_rows(&case),
            mode: NamedInsertMode::IncrementingLast(IncrementingAutoNumber::new(29)),
            clock: Some(&clock),
        },
        &mut budget(),
        &|| false,
    );
    assert!(result.is_err());
    assert_eq!(clock.0.load(Ordering::Relaxed), 0);
}

#[test]
fn fully_supplied_named_clock_batch_uses_complete_path_without_reading_provider() {
    let case = model::reordered(6, true, Kind::Clock, 2);
    let fields = case
        .fields
        .iter()
        .map(|field| field.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let mut rows = original_rows(&case);
    rows[0][0] = InsertFieldValue::Supplied(Value::DateTime(
        LegacyDateTime::from_bits(0x40e1dd3000000000).unwrap(),
    ));
    let clock = CountClock(AtomicUsize::new(0));
    let copy = direct(
        &case.before,
        EngineeringRequest::NamedInsert {
            table: "Items",
            fields: &fields,
            rows,
            mode: NamedInsertMode::Incrementing,
            clock: Some(&clock),
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    same_image(copy.candidate_bytes(), &case.expected);
    assert_eq!(clock.0.load(Ordering::Relaxed), 0);
}

#[test]
fn named_cli_failures_keep_source_requests_and_output_unchanged() {
    let case = model::missing_other(6, false, Kind::Integer);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    let requests = directory.path().join("requests.tsv");
    fs::write(&input, &case.before).unwrap();
    for (ordinal, (fields, contents)) in [
        (vec!["Id", "id"], "int32\t4\tint32\t40\n"),
        (vec!["Missing"], "int32\t4\n"),
        (vec!["Id", "Score"], "int32\t4\n"),
        (vec!["Id", "Score"], "int32\t4\tnull\t\n"),
        (
            vec!["Id", "Score"],
            "int32\t4\tint32\t40\nint32\t5\tint32\t10\n",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused{ordinal}.accdb"));
        fs::write(&requests, contents).unwrap();
        let result = command(&input, &output, &requests, &fields, &[]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        same_image(&fs::read(&input).unwrap(), &case.before);
        assert_eq!(fs::read_to_string(&requests).unwrap(), contents);
    }
}

#[test]
fn named_generated_and_needed_clock_refusals_do_not_fallback() {
    let case = model::missing_other(6, true, Kind::Clock);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    let requests = directory.path().join("requests.tsv");
    fs::write(&input, &case.before).unwrap();
    for (ordinal, (fields, contents, flags)) in [
        (
            vec!["Score"],
            "int32\t31\n",
            vec!["--incrementing", CLOCK_FLAG],
        ),
        (
            vec!["Id"],
            "int32\t4\n",
            vec!["--incrementing-last", "29", CLOCK_FLAG],
        ),
        (vec!["Id"], "int32\t4\n", vec!["--incrementing"]),
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused{ordinal}.accdb"));
        fs::write(&requests, contents).unwrap();
        let result = command(&input, &output, &requests, &fields, &flags);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        same_image(&fs::read(&input).unwrap(), &case.before);
        assert_eq!(fs::read_to_string(&requests).unwrap(), contents);
    }
}

#[test]
fn named_capture_mapping_finish_share_exact_pools_and_final_cancellation() {
    let case = model::missing_other(6, false, Kind::Integer);
    let fields = case
        .fields
        .iter()
        .map(|field| (*field).to_owned())
        .collect::<Vec<_>>();
    let ceilings = [256 * 1024 * 1024, 100_000, 100_000_000];
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let mut baseline = ResourceBudget::new(ceilings[0], ceilings[1], ceilings[2]);
    let calls = Cell::new(0);
    let prepare = |work: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
        direct(
            &case.before,
            EngineeringRequest::NamedInsert {
                table: "Items",
                fields: &fields,
                rows: original_rows(&case),
                mode: NamedInsertMode::Ordinary,
                clock: None,
            },
            work,
            cancelled,
        )
    };
    prepare(&mut baseline, &|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let used =
        std::array::from_fn::<_, 3, _>(|index| ceilings[index] - baseline.remaining(pools[index]));
    let mut exact = ResourceBudget::new(used[0], used[1], used[2]);
    same_image(
        prepare(&mut exact, &|| false).unwrap().candidate_bytes(),
        &case.expected,
    );
    for index in 0..3 {
        let mut limits = used;
        limits[index] -= 1;
        let error = prepare(
            &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
            &|| false,
        )
        .err()
        .unwrap();
        let error = NativeEditError::from(error);
        assert!(
            matches!(error, NativeEditError::Inspect(InspectError::Limit(error)) | NativeEditError::Edit(EditError::Inspect(InspectError::Limit(error))) if error.resource == pools[index])
        );
    }
    let total = calls.get();
    calls.set(0);
    let error = prepare(&mut ResourceBudget::new(used[0], used[1], used[2]), &|| {
        calls.set(calls.get() + 1);
        calls.get() == total
    })
    .err()
    .unwrap();
    assert!(matches!(
        NativeEditError::from(error),
        NativeEditError::Inspect(InspectError::Cancelled)
            | NativeEditError::Edit(EditError::Inspect(InspectError::Cancelled))
    ));
}
