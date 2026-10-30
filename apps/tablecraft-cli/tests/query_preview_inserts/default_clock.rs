//! Fixed civil inputs and literal legacy DATE bits, before clock-enabled actions.
use super::{PAGE, budget, checked, expected, fs, omissions, support};
use std::{
    io::Cursor,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_access::engineering::{ScalarInsertMode, sql_insert};
use tablecraft_ace::{NativeImage, inspect_table_properties, inspect_table_schema, read_table};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

const DATE_BITS: u64 = 0x40e1dd2000000000;
const NOW_BITS: u64 = 0x40e1dd3000000000;
const FIXED_ARGUMENT: &str = "--default-clock=2000-02-29T12:00:00.7654321";
const SOURCE_ROWS: [(i32, i32, Option<u64>); 3] = [
    (1, 10, Some(0)),
    (2, 20, None),
    (3, 30, Some(0xbff4000000000000)),
];

pub(super) fn source(code: u8, generated: bool, expression: &str) -> Vec<u8> {
    let mut before = omissions::three_field_source(code, generated);
    let column = 10 * PAGE + 75 + 2 * 25;
    assert_eq!(before[column], 4);
    before[column] = 8;
    support::put16(&mut before, column + 23, 8);
    omissions::set_other_default(&mut before, expression);
    before[12 * PAGE..13 * PAGE].copy_from_slice(&date_page(&SOURCE_ROWS));
    before
}

fn date_page(rows: &[(i32, i32, Option<u64>)]) -> Vec<u8> {
    let records = rows
        .iter()
        .map(|(id, score, other)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(score.to_le_bytes());
            row.extend(other.map_or([0xa5; 8], u64::to_le_bytes));
            row.push(3 | if other.is_some() { 4 } else { 0 });
            assert_eq!(row.len(), 19);
            (0, row)
        })
        .collect::<Vec<_>>();
    let mut page = support::data_page(10, &records);
    support::put16(&mut page, 2, (PAGE - 14 - rows.len() * 21) as u16);
    page
}

fn wanted(before: &[u8], generated: bool, other: Option<u64>) -> Vec<u8> {
    let mut after = expected(before, generated);
    let mut records = SOURCE_ROWS.to_vec();
    records.push((4, if generated { 31 } else { 40 }, other));
    // Retain old opaque NULL cells while the new absent payload is canonical0.
    let mut page = date_page(&records);
    if other.is_none() {
        let start = usize::from(u16::from_le_bytes(page[20..22].try_into().unwrap()));
        page[start + 10..start + 18].fill(0);
    }
    after[12 * PAGE..13 * PAGE].copy_from_slice(&page);
    after
}

fn sql(generated: bool, supplied: bool) -> &'static str {
    match (generated, supplied) {
        (false, false) => "INSERT INTO Items(Id,Score) VALUES(4,40)",
        (true, false) => "INSERT INTO Items(Id) VALUES(4)",
        (false, true) => "INSERT INTO Items(Id,Score,Other) VALUES(4,40,NULL)",
        (true, true) => "INSERT INTO Items(Id,Other) VALUES(4,NULL)",
    }
}

fn mode(generated: bool) -> ScalarInsertMode {
    if generated {
        ScalarInsertMode::Incrementing
    } else {
        ScalarInsertMode::Explicit
    }
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

fn qualify(before: &[u8], after: &[u8], generated: bool, value: Option<u64>) {
    checked(before, 3);
    checked(after, 4);
    let schema = inspect_table_schema(
        &mut Cursor::new(before),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|field| (
                field.name(),
                field.number(),
                field.native_type(),
                field.storage_length()
            ))
            .collect::<Vec<_>>(),
        [("Id", 0, 4, 4), ("Score", 1, 4, 4), ("Other", 2, 8, 8)]
    );
    assert_eq!(schema.columns()[1].flags(), if generated { 7 } else { 3 });
    let table = read_table(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows()[3],
        [
            Value::Int32(4),
            Value::Int32(if generated { 31 } else { 40 }),
            value.map_or(Value::Null, |bits| Value::DateTime(
                LegacyDateTime::from_bits(bits).unwrap()
            ))
        ]
    );
    assert_eq!(&before[4 * PAGE..5 * PAGE], &after[4 * PAGE..5 * PAGE]);
    for ordinal in 0..3 {
        let at = 12 * PAGE + 14 + ordinal * 2;
        let start = usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()));
        assert_eq!(
            &before[12 * PAGE + start..12 * PAGE + start + 19],
            &after[12 * PAGE + start..12 * PAGE + start + 19]
        );
    }
}

#[test]
fn qualify_default_clock_frontend_sources_and_literal_whole_images_before_actions() {
    for code in [5, 6] {
        for generated in [false, true] {
            for (expression, bits) in [("Date()", DATE_BITS), ("Now()", NOW_BITS)] {
                let before = source(code, generated, expression);
                let after = wanted(&before, generated, Some(bits));
                qualify(&before, &after, generated, Some(bits));
                let properties = inspect_table_properties(
                    &mut Cursor::new(&before),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    properties
                        .groups()
                        .iter()
                        .find(|group| group.name() == "Other")
                        .unwrap()
                        .get("DefaultValue")
                        .unwrap()
                        .text()
                        .unwrap(),
                    expression
                );
            }
            let before = source(code, generated, "Now()");
            qualify(&before, &wanted(&before, generated, None), generated, None);
        }
        let unknown = source(code, false, "Now()+1");
        qualify(&unknown, &wanted(&unknown, false, None), false, None);
        let mut maximum = source(code, true, "Now()");
        support::put32(&mut maximum, 10 * PAGE + 20, i32::MAX as u32);
        checked(&maximum, 3);
        let mut wrong = omissions::three_field_source(code, false);
        omissions::set_other_default(&mut wrong, "Now()");
        checked(&wrong, 3);
    }
}

struct Provider {
    calls: AtomicUsize,
    fail: bool,
}

impl Provider {
    fn new(fail: bool) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail,
        }
    }
}

impl Clock for Provider {
    fn read(&self) -> Result<ClockReading, EvalError> {
        let calls = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
        if self.fail || calls > 1 {
            return Err(EvalError::ClockUnavailable);
        }
        ClockReading::new(ExtendedDateTime::new(2000, 2, 29, 12, 0, 0, 7_654_321).unwrap())
    }
}

#[test]
fn access_clock_provider_is_lazy_needed_once_and_original_reading_is_not_replayed() {
    for code in [5, 6] {
        for generated in [false, true] {
            for (expression, bits) in [("Date()", DATE_BITS), ("Now()", NOW_BITS)] {
                let before = source(code, generated, expression);
                let query = parse_query(sql(generated, false), ParseLimits::default()).unwrap();
                let provider = Provider::new(false);
                let copy = sql_insert::preview_insert_with_omissions_and_clock(
                    capture(&before),
                    query.query(),
                    mode(generated),
                    &provider,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(provider.calls.load(Ordering::Relaxed), 1);
                assert_eq!(copy.before_bytes(), before);
                assert_eq!(
                    copy.candidate_bytes(),
                    wanted(&before, generated, Some(bits))
                );
            }
            let before = source(code, generated, "Now()");
            let query = parse_query(sql(generated, true), ParseLimits::default()).unwrap();
            let provider = Provider::new(true);
            let copy = sql_insert::preview_insert_with_omissions_and_clock(
                capture(&before),
                query.query(),
                mode(generated),
                &provider,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(provider.calls.load(Ordering::Relaxed), 0);
            assert_eq!(copy.candidate_bytes(), wanted(&before, generated, None));
        }
    }
}

#[test]
fn clock_source_binding_invalid_defaults_targets_and_maximum_never_read_provider() {
    for code in [5, 6] {
        let mut maximum = source(code, true, "Now()");
        support::put32(&mut maximum, 10 * PAGE + 20, i32::MAX as u32);
        let mut wrong = omissions::three_field_source(code, false);
        omissions::set_other_default(&mut wrong, "Now()");
        for (before, generated, statement) in [
            (source(code, false, "Now()+1"), false, sql(false, false)),
            (wrong, false, sql(false, false)),
            (maximum, true, sql(true, false)),
            (
                source(code, false, "Now()"),
                false,
                "INSERT INTO Items(Id,ID) VALUES(4,5)",
            ),
        ] {
            let provider = Provider::new(true);
            let query = parse_query(statement, ParseLimits::default()).unwrap();
            let result = sql_insert::preview_insert_with_omissions_and_clock(
                capture(&before),
                query.query(),
                mode(generated),
                &provider,
                &mut budget(),
                &|| false,
            );
            if generated {
                assert!(matches!(
                    result,
                    Err(tablecraft_query::InsertError::Backend(
                        tablecraft_access::NativeEditError::Edit(
                            tablecraft_ace::EditError::AutoNumberExhausted
                        )
                    ))
                ));
            } else {
                assert!(result.is_err());
            }
            assert_eq!(provider.calls.load(Ordering::Relaxed), 0);
        }
        let before = source(code, false, "Now()");
        let query = parse_query(sql(false, false), ParseLimits::default()).unwrap();
        let provider = Provider::new(true);
        assert!(matches!(
            sql_insert::preview_insert_with_omissions_and_clock(
                capture(&before),
                query.query(),
                mode(false),
                &provider,
                &mut budget(),
                &|| false
            ),
            Err(tablecraft_query::InsertError::Backend(
                tablecraft_access::NativeEditError::Edit(
                    tablecraft_ace::EditError::DefaultEvaluation(EvalError::ClockUnavailable)
                )
            ))
        ));
        assert_eq!(provider.calls.load(Ordering::Relaxed), 1);
    }
}

fn run(
    input: &Path,
    output: &Path,
    statement: &str,
    generated: bool,
    clock: Option<&str>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["query", "preview-insert", statement, "--database"])
        .arg(input)
        .arg("--output")
        .arg(output);
    if generated {
        command.arg("--incrementing");
    }
    if let Some(clock) = clock {
        command.arg(clock);
    }
    command.output().unwrap()
}

#[test]
fn explicit_default_clock_cli_fixed_reading_and_bare_supplied_null_match_whole_images() {
    for code in [5, 6] {
        for generated in [false, true] {
            for (expression, bits) in [("Date()", DATE_BITS), ("Now()", NOW_BITS)] {
                let before = source(code, generated, expression);
                let directory = tempfile::tempdir().unwrap();
                let input = directory.path().join("before.accdb");
                let output = directory.path().join("after.accdb");
                fs::write(&input, &before).unwrap();
                let result = run(
                    &input,
                    &output,
                    sql(generated, false),
                    generated,
                    Some(FIXED_ARGUMENT),
                );
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(
                    fs::read(output).unwrap(),
                    wanted(&before, generated, Some(bits))
                );
                assert_eq!(fs::read(input).unwrap(), before);
            }
            let before = source(code, generated, "Now()");
            let directory = tempfile::tempdir().unwrap();
            let input = directory.path().join("before.accdb");
            let output = directory.path().join("after.accdb");
            fs::write(&input, &before).unwrap();
            let result = run(
                &input,
                &output,
                sql(generated, true),
                generated,
                Some("--default-clock"),
            );
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(fs::read(output).unwrap(), wanted(&before, generated, None));
            assert_eq!(fs::read(input).unwrap(), before);
        }
    }
}

#[test]
fn absent_clock_invalid_override_and_values_calls_publish_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    let output = directory.path().join("after.accdb");
    let before = source(6, false, "Now()");
    fs::write(&input, &before).unwrap();
    let result = run(&input, &output, sql(false, false), false, None);
    assert_eq!(result.status.code(), Some(2));
    assert!(!output.exists());
    let missing = directory.path().join("missing.accdb");
    for (statement, clock) in [
        (sql(false, false), "--default-clock=invalid"),
        (
            "INSERT INTO Items(Id,Score) VALUES(4,Now())",
            "--default-clock",
        ),
    ] {
        let result = run(&missing, &output, statement, false, Some(clock));
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("NotFound"));
        assert!(!output.exists());
    }
    let separated = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-insert", sql(false, false), "--database"])
        .arg(&missing)
        .arg("--output")
        .arg(&output)
        .args(["--default-clock", "2000-02-29T12:00:00"])
        .output()
        .unwrap();
    assert_eq!(separated.status.code(), Some(2));
    assert!(separated.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&separated.stderr).contains("NotFound"));
    assert!(!output.exists());
    assert_eq!(fs::read(input).unwrap(), before);
}
