use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::conversions::text::NumberFormat,
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{parse_query, parse_select, select::Projection};

struct Case {
    name: &'static str,
    arguments: Vec<&'static str>,
    array_index: Option<usize>,
    expected: f64,
}

fn cases() -> Vec<Case> {
    let mut cases: Vec<_> = [
        ("Pmt", vec!["0", "2", "100", "0", "0"], -50.0),
        ("PV", vec!["0", "2", "-50", "0", "0"], 100.0),
        ("FV", vec!["0", "2", "-50", "0", "0"], 100.0),
        ("NPer", vec!["0", "-50", "100", "0", "0"], 2.0),
        ("IPmt", vec!["0", "1", "2", "100", "0", "0"], 0.0),
        ("PPmt", vec!["0", "1", "2", "100", "0", "0"], -50.0),
        ("SLN", vec!["100", "0", "10"], 10.0),
        ("SYD", vec!["100", "0", "4", "1"], 40.0),
        ("DDB", vec!["1000", "0", "5", "1", "2"], 400.0),
        ("Rate", vec!["1", "-110", "100", "0", "0", "0.1"], 0.1),
        (
            "EuroConvert",
            vec!["1.95583", "DEM", "EUR", "1", "3.5"],
            1.0,
        ),
    ]
    .into_iter()
    .map(|(name, arguments, expected)| Case {
        name,
        arguments,
        array_index: None,
        expected,
    })
    .collect();
    cases.extend([
        Case {
            name: "NPV",
            arguments: vec!["0"],
            array_index: Some(1),
            expected: 21.0,
        },
        Case {
            name: "IRR",
            arguments: vec!["0.1"],
            array_index: Some(0),
            expected: 0.1,
        },
        Case {
            name: "MIRR",
            arguments: vec!["0", "0"],
            array_index: Some(0),
            expected: 0.1,
        },
    ]);
    cases
}

impl Case {
    fn sql(&self, format: NumberFormat, first_control: Option<&str>) -> String {
        let mut arguments: Vec<_> = self
            .arguments
            .iter()
            .map(|text| {
                let text = if format == NumberFormat::DeDe {
                    text.replace('.', ",")
                } else {
                    (*text).to_owned()
                };
                format!("CVar(IIf(True,Mid('{text}',1),'unused'))")
            })
            .collect();
        if let Some(input) = first_control {
            arguments[0] = input.to_owned();
        }
        if let Some(index) = self.array_index {
            arguments.insert(index, "Array(-100,0,121)".into());
        }
        format!("SELECT {}({})", self.name, arguments.join(","))
    }
}

fn assert_number(actual: &Value, expected: f64) {
    let Value::Double(actual) = actual else {
        panic!("Double result required");
    };
    assert!(
        (actual.to_float() - expected).abs() <= 1e-10,
        "{} != {expected}",
        actual.to_float()
    );
}

#[test]
fn all_fourteen_financial_calls_keep_scalar_and_bound_retained_results() {
    let source = MemorySource::default();
    for case in cases() {
        for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
            let sql = case.sql(format, None);
            let scalar = parse_select(&sql, Default::default()).unwrap();
            let Projection::Expression { expression, .. } = &scalar.query().projection[0] else {
                panic!("scalar projection required");
            };
            let mut budget = ResourceBudget::new(65536, 0, 100000);
            let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .evaluate(expression)
                .unwrap();
            assert_number(&result, case.expected);

            let query = parse_query(&sql, Default::default()).unwrap();
            let result = prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(format),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_number(&result.rows()[0][0], case.expected);
        }
    }
}

#[test]
fn all_financial_query_paths_keep_raw_refusals_and_eager_errors() {
    let source = MemorySource::default();
    for case in cases() {
        for (input, expected) in [
            ("ChrW(55296)", EvalError::UnrepresentableText),
            ("ChrB(49)", EvalError::UnrepresentableText),
            ("IIf(True,'1',1/0)", EvalError::DivisionByZero),
        ] {
            let sql = case.sql(NumberFormat::EnUs, Some(input));
            let query = parse_query(&sql, Default::default()).unwrap();
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
            assert_eq!(error, Some(expected.into()), "{} {input}", case.name);
        }
    }
}
