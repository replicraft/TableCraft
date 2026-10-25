use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn constructors_keep_query_results_with_retained_numeric_components() {
    let source = MemorySource::default();

    for (call, expected) in [
        (
            "DateSerial(CVar(Mid('2024',1)),IIf(True,'2','unused'),'29')",
            LegacyDateTime::from_calendar("2024-02-29T00:00:00".parse().unwrap()).unwrap(),
        ),
        (
            "TimeSerial(CVar(Mid('-1',1)),IIf(True,'30','unused'),'0')",
            LegacyDateTime::from_ole_days(-1.0 / 48.0).unwrap(),
        ),
    ] {
        let query = parse_query(&format!("SELECT {call}"), Default::default()).unwrap();
        let result = prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(result.rows(), &[vec![Value::DateTime(expected)]]);
    }
}

#[test]
fn constructors_keep_raw_refusals_and_eager_operand_failures_in_queries() {
    let source = MemorySource::default();

    for function in ["DateSerial", "TimeSerial"] {
        for (argument, expected) in [
            ("ChrW(55296)", EvalError::UnrepresentableText),
            ("ChrB(49)", EvalError::UnrepresentableText),
            ("IIf(True,'1',1/0)", EvalError::DivisionByZero),
        ] {
            let query = parse_query(
                &format!("SELECT {function}({argument},'1','1')"),
                Default::default(),
            )
            .unwrap();
            let error = prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(NumberFormat::EnUs),
                        ..Default::default()
                    },
                )
                .err();

            assert_eq!(error, Some(expected.into()));
        }
    }
}
