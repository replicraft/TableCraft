use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn character_and_reverse_calls_rebind_formats_and_keep_integer_code_results() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE; SELECT Asc(N),AscW(N),(SELECT StrReverse(N)),Asc(65)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("N", Value::Double(Double::from_float(12.5)))];
    for (format, reversed) in [(NumberFormat::EnUs, "5.21"), (NumberFormat::DeDe, "5,21")] {
        assert_eq!(
            plan.execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![
                Value::Int16(49),
                Value::Int16(49),
                Value::Text(reversed.into()),
                Value::Int16(54)
            ]]
        );
    }
    assert!(
        plan.execute(&source, &bindings, Default::default())
            .is_err()
    );
}

#[test]
fn null_and_eager_failures_remain_errors_without_implicit_formatting_policy() {
    let source = MemorySource::default();
    for function in ["Asc", "AscW", "StrReverse"] {
        for operand in ["NULL", "1/0"] {
            let parsed =
                parse_query(&format!("SELECT {function}({operand})"), Default::default()).unwrap();
            assert!(matches!(
                prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(&source, &[], Default::default()),
                Err(QueryError::Evaluation(_))
            ));
        }
    }
}
