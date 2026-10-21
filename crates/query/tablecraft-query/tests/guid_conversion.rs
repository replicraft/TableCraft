use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn guid_byte_results_keep_correlations_parameters_and_aggregate_dependencies() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT GUIDFromString(P),(SELECT GUIDFromString(P))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (text, expected) in [
        ("00000000-0000-0000-0000-000000000000", vec![0; 16]),
        (
            "00112233-4455-6677-8899-AABBCCDDEEFF",
            vec![
                0x33, 0x22, 0x11, 0, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ],
        ),
    ] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(text.into()))],
                Default::default(),
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Binary(expected.clone()),
                Value::Binary(expected)
            ]]
        );
    }

    let parsed = parse_query(
        "SELECT GUIDFromString(First(A.G)) FROM (SELECT '00112233-4455-6677-8899-AABBCCDDEEFF' AS G) AS A",
        Default::default(),
    ).unwrap();
    assert!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .is_ok()
    );

    let parsed = parse_query("SELECT GUIDFromString(1/0)", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn every_observed_guid_conversion_checkpoint_can_cancel() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT GUIDFromString('00112233-4455-6677-8899-AABBCCDDEEFF')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
