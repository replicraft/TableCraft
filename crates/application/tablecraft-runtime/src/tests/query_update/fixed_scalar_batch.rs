//! Descriptor-bearing batches keep native types even when every selected value is NULL.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Cell, CellKind, Command, Error, ParameterInput, Response};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_display::{DateRepresentation, ScalarPresentation};
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fixed_scalars::{
    self as raw, Column, Scalar, batch,
};

const LARGE_INPUT: &str = "9007199254740995";

struct Case {
    original: [Option<Scalar>; 2],
    values: [Option<Scalar>; 2],
    sql: &'static str,
    singleton_sql: &'static str,
}

fn case(column: Column) -> Case {
    let (original, values, sql, singleton_sql) = match column {
        Column::ByteValue => (
            Scalar::Byte(250),
            [Scalar::Byte(251), Scalar::Byte(1)],
            "UPDATE Measurements SET ByteValue=CByte(Nz(ByteValue,0)+1)",
            "UPDATE Measurements SET ByteValue=CByte(251) WHERE ID=1",
        ),
        Column::ShortValue => (
            Scalar::Int16(-10),
            [Scalar::Int16(-8), Scalar::Int16(2)],
            "UPDATE Measurements SET ShortValue=CInt(Nz(ShortValue,0)+2)",
            "UPDATE Measurements SET ShortValue=CInt(-8) WHERE ID=1",
        ),
        Column::LargeValue => (
            Scalar::Int64(9_007_199_254_740_993),
            [Scalar::Int64(9_007_199_254_740_995), Scalar::Int64(1)],
            "PARAMETERS [Large] BIGINT; UPDATE Measurements SET LargeValue=IIf(ID=1,[Large],CLngLng(1))",
            "PARAMETERS [Large] BIGINT; UPDATE Measurements SET LargeValue=[Large] WHERE ID=1",
        ),
        Column::SingleValue => (
            Scalar::SingleBits(0xc020_0000),
            [
                Scalar::SingleBits(0xbfa0_0000),
                Scalar::SingleBits(0x3fa0_0000),
            ],
            "UPDATE Measurements SET SingleValue=CSng(Nz(SingleValue,0)+1.25)",
            "UPDATE Measurements SET SingleValue=CSng(-1.25) WHERE ID=1",
        ),
        Column::DoubleValue => (
            Scalar::DoubleBits(0xc004_0000_0000_0000),
            [
                Scalar::DoubleBits(0xbff4_0000_0000_0000),
                Scalar::DoubleBits(0x3ff4_0000_0000_0000),
            ],
            "UPDATE Measurements SET DoubleValue=CDbl(Nz(DoubleValue,0)+1.25)",
            "UPDATE Measurements SET DoubleValue=CDbl(-1.25) WHERE ID=1",
        ),
        Column::DateValue => (
            Scalar::DateTimeBits(0xc004_0000_0000_0000),
            [
                Scalar::DateTimeBits(0xbff8_0000_0000_0000),
                Scalar::DateTimeBits(0),
            ],
            "UPDATE Measurements SET DateValue=DateAdd('d',1,Nz(DateValue,#1899-12-29#))",
            "UPDATE Measurements SET DateValue=DateAdd('d',1,#1899-12-28T12:00:00#) WHERE ID=1",
        ),
    };

    Case {
        original: [Some(original), None],
        values: values.map(Some),
        sql,
        singleton_sql,
    }
}

fn inputs(column: Column) -> Vec<ParameterInput> {
    if matches!(column, Column::LargeValue) {
        vec![ParameterInput {
            name: "Large".into(),
            kind: "int64".into(),
            text: LARGE_INPUT.into(),
            is_null: false,
        }]
    } else {
        Vec::new()
    }
}

fn observe(cell: &Cell, expected: Option<Scalar>) {
    let CellKind::Scalar { is_null, .. } = cell.kind else {
        panic!("native scalar observation");
    };
    assert_eq!(is_null, expected.is_none());

    match expected {
        None => assert_eq!(cell.text, "NULL"),
        Some(Scalar::Byte(value)) => assert_eq!(cell.text, value.to_string()),
        Some(Scalar::Int16(value)) => assert_eq!(cell.text, value.to_string()),
        Some(Scalar::Int64(value)) => assert_eq!(cell.text, value.to_string()),
        Some(Scalar::SingleBits(bits)) => assert_eq!(cell.text, f32::from_bits(bits).to_string()),
        Some(Scalar::DoubleBits(bits)) => assert_eq!(cell.text, f64::from_bits(bits).to_string()),
        Some(Scalar::DateTimeBits(bits)) => assert!(matches!(
            cell.presentation,
            Some(ScalarPresentation::DateTime {
                representation: DateRepresentation::LegacyBits(actual),
                ..
            }) if actual == bits
        )),
    }
}

fn run_case(column: Column) {
    let case = case(column);
    let source = batch::values(column, &case.original, &case.values);
    let (_directory, path, worker, mut revision) = opened(&source.before);
    let mut expected = source.before;
    for (phase, (sql, parameters, values, matched, changed)) in [
        (case.sql.to_string(), inputs(column), case.values, 2, 2),
        (
            format!("UPDATE Measurements SET [{}]=NULL", column.name()),
            Vec::new(),
            [None, None],
            2,
            2,
        ),
        (
            format!(
                "UPDATE Measurements SET [{}]=NULL WHERE ID<0",
                column.name()
            ),
            Vec::new(),
            [None, None],
            0,
            0,
        ),
        (
            case.singleton_sql.to_string(),
            inputs(column),
            [case.values[0], None],
            1,
            1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let next = raw::update(&expected, source.definition, 0, column, values[0]);
        let next = raw::update(&next, source.definition, 1, column, values[1]);
        if phase == 0 {
            assert_image(&next, &source.expected);
        }
        let (target, specs) = inspect(&worker, revision, &sql);
        if !parameters.is_empty() {
            assert_eq!(specs.len(), 1);
            assert_eq!(specs[0].name, "Large");
            assert_eq!(specs[0].kind, "int64");
        } else {
            assert!(specs.is_empty());
        }
        let (stale, _) = inspect(&worker, revision, &sql);
        let Response::QueryUpdateReviewed { review, capability } =
            request(&worker, Command::ReviewQueryUpdate { target, parameters })
                .unwrap_or_else(|error| panic!("fixed scalar {}: {sql}: {error:?}", column.name()))
        else {
            panic!("one native fixed-scalar review");
        };
        assert_eq!(
            (review.matched_rows, review.changed_rows),
            (matched, changed)
        );
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(!crate::journal_path(&path).exists());
        let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(matched), Some(changed))
        );
        assert!(receipt.schema_change.is_none());
        assert_image(&std::fs::read(&path).unwrap(), &next);
        let Response::Opened {
            document_revision, ..
        } = request(&worker, open(&path, "")).unwrap()
        else {
            panic!("fresh fixed-scalar source");
        };
        revision = document_revision;
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryUpdate {
                    target: stale,
                    parameters: Vec::new()
                }
            ),
            Err(Error::StaleQuery)
        ));
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Query {
                options: Default::default(),
                sql: format!("SELECT [{}] FROM Measurements ORDER BY ID", column.name()),
            },
        )
        .unwrap() else {
            panic!("fresh typed source values");
        };
        assert_eq!(rows.len(), 2);
        for (row, value) in rows.iter().zip(values) {
            observe(&row[0], value);
        }
        expected = next;
    }
}

#[test]
fn byte_batch_null_empty_and_old_singleton_keep_exact_native_bytes() {
    run_case(Column::ByteValue);
}

#[test]
fn int16_batch_null_empty_and_old_singleton_keep_exact_native_bytes() {
    run_case(Column::ShortValue);
}

#[test]
fn int64_batch_uses_declared_exact_inputs_beyond_double_integer_precision() {
    run_case(Column::LargeValue);
}

#[test]
fn single_batch_null_empty_and_old_singleton_keep_exact_native_bits() {
    run_case(Column::SingleValue);
}

#[test]
fn double_batch_null_empty_and_old_singleton_keep_exact_native_bits() {
    run_case(Column::DoubleValue);
}

#[test]
fn legacy_date_batch_null_empty_and_old_singleton_keep_ole_bits() {
    run_case(Column::DateValue);
}

#[test]
fn later_byte_overflow_preserves_every_original_row_and_companion() {
    let source = batch::values(
        Column::ByteValue,
        &[Some(Scalar::Byte(17)), None],
        &[Some(Scalar::Byte(17)), None],
    );
    let (_directory, path, worker, revision) = opened(&source.before);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Measurements SET ByteValue=CByte(IIf(ID=1,1,256))",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(EvalError::Overflow))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &source.before);
    assert!(!crate::journal_path(&path).exists());
}
