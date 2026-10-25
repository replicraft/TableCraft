use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Single, Value},
};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{parse_query, parse_select, select::Projection};

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

fn cases() -> Vec<(&'static str, Vec<&'static str>, Value)> {
    vec![
        ("Abs", vec!["-2.5"], double(2.5)),
        ("Sgn", vec!["-2.5"], Value::Int16(-1)),
        ("Int", vec!["-2.5"], double(-3.0)),
        ("Fix", vec!["-2.5"], double(-2.0)),
        ("Sqr", vec!["4"], double(2.0)),
        ("Exp", vec!["0"], double(1.0)),
        ("Log", vec!["1"], double(0.0)),
        ("Sin", vec!["0"], double(0.0)),
        ("Cos", vec!["0"], double(1.0)),
        ("Tan", vec!["0"], double(0.0)),
        ("Atn", vec!["0"], double(0.0)),
        ("Round", vec!["2.5", "0"], double(2.0)),
        ("RGB", vec!["1", "2", "3"], Value::Int32(197121)),
        ("QBColor", vec!["2.5"], Value::Int32(32768)),
        (
            "Rnd",
            vec!["0"],
            Value::Single(Single::from_float(0.01953125)),
        ),
    ]
}

#[test]
fn every_math_call_keeps_scalar_and_bound_retained_composition_results() {
    let source = MemorySource::default();
    for (name, arguments, expected) in cases() {
        for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
            let arguments = arguments
                .iter()
                .map(|text| {
                    let text = if format == NumberFormat::DeDe {
                        text.replace('.', ",")
                    } else {
                        (*text).to_owned()
                    };
                    format!("CVar(IIf(True,Mid('{text}',1),'unused'))")
                })
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!("SELECT {name}({arguments})");
            let scalar = parse_select(&sql, Default::default()).unwrap();
            let Projection::Expression { expression, .. } = &scalar.query().projection[0] else {
                panic!("scalar projection required");
            };
            let state = RndState::default();
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .with_random(&state)
                .evaluate(expression);
            assert_eq!(result, Ok(expected.clone()), "scalar {name}");

            let query = parse_query(&sql, Default::default()).unwrap();
            let state = RndState::default();
            let result = prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(format),
                        random: Some(&state),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(result.rows()[0][0], expected, "bound {name}");
        }
    }
}

#[test]
fn every_bound_math_call_retains_raw_refusals_and_eager_operational_errors() {
    let source = MemorySource::default();
    for (name, arguments, _) in cases() {
        for (input, expected) in [
            ("ChrW(55296)", EvalError::UnrepresentableText),
            ("ChrB(49)", EvalError::UnrepresentableText),
            ("IIf(True,'1',1/0)", EvalError::DivisionByZero),
        ] {
            let mut arguments: Vec<_> = arguments.iter().map(|text| format!("'{text}'")).collect();
            arguments[0] = input.to_owned();
            let query = parse_query(
                &format!("SELECT {name}({})", arguments.join(",")),
                Default::default(),
            )
            .unwrap();
            let state = RndState::default();
            let error = prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(NumberFormat::EnUs),
                        random: Some(&state),
                        ..Default::default()
                    },
                )
                .err();
            assert_eq!(error, Some(expected.into()), "{name} {input}");
            assert_eq!(
                state.apply(Some(&Value::Int16(0))),
                Ok(Value::Single(Single::from_float(0.01953125)))
            );
        }
    }
}
