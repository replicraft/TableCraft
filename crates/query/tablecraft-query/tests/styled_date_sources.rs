use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn date_sources_rebind_raw_serials_without_a_calendar_output_profile() {
    let source = MemorySource::default();
    let query = parse_query(
        concat!(
            "PARAMETERS D DATE; SELECT FormatNumber(D,'2,5'),",
            "FormatPercent(D),FormatCurrency(D),",
            "(SELECT FormatNumber(D)),Eval('FormatNumber(CDate(-0.25))')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (serial, number, percent, currency) in [
        (-1.25, "-1,25", "-125,00%", "-1,25 €"),
        (-0.25, "-0,25", "-25,00%", "-0,25 €"),
        (0.25, "0,25", "25,00%", "0,25 €"),
    ] {
        let bindings = [(
            "D",
            Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap()),
        )];
        let options = ExecutionOptions {
            number_format: Some(NumberFormat::DeDe),
            ..Default::default()
        };
        assert_eq!(
            plan.execute(&source, &bindings, options).unwrap().rows(),
            [vec![
                Value::Text(number.into()),
                Value::Text(percent.into()),
                Value::Text(currency.into()),
                Value::Text(number.into()),
                Value::Text("-0,25".into()),
            ]]
        );
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("D", Value::Null)],
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
}
