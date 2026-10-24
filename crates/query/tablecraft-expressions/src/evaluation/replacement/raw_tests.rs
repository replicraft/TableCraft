use crate::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::{conversions::text::NumberFormat, text::byte_string::ByteOperand},
};
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, ExpressionValue, Value},
};
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .evaluate(&expression)
}

pub(super) fn raw(units: &[u16]) -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(
        units.iter().flat_map(|unit| unit.to_le_bytes()).collect(),
    ))
}

pub(super) fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn replacement_retains_raw_units_and_can_repair_final_unicode() {
    for (sql, expected) in [
        (
            "LenB(Replace(ChrW(55296)&'ab'&ChrW(55296),ChrW(55296),ChrW(57343)))",
            Value::Int32(8),
        ),
        (
            "AscW(Replace(ChrW(55296),ChrW(55296),ChrW(57343)))",
            Value::Int16(-8193),
        ),
        (
            "StrComp(Replace(ChrW(55296)&'a',ChrW(55296),ChrW(57343)),ChrW(57343)&'a',0)",
            Value::Int16(0),
        ),
        (
            "Replace(ChrW(55296)&'a','a',ChrW(56320))",
            Value::Text("\u{10000}".into()),
        ),
        ("AscW(Replace('😀',ChrW(55357),''))", Value::Int16(-8704)),
        ("LenB(Replace('😀','z','',2,0))", Value::Int32(2)),
        (
            "StrComp(Replace(MidB('😀',2,2),ChrW(216),ChrW(65)),'A',0)",
            Value::Int16(0),
        ),
        ("LenB(Replace(ChrW(55296),'a','b',1,0,-1))", Value::Int32(2)),
        ("LenB(Replace(ChrW(55296),'',ChrW(57343)))", Value::Int32(2)),
        ("Replace(12,ChrW(50),'X')", Value::Text("1X".into())),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
}

#[test]
fn raw_replacement_keeps_controls_admission_eagerness_and_strict_publication() {
    for (sql, expected) in [
        ("Replace(ChrW(55296),'a','b',0)", EvalError::Domain),
        ("Replace(ChrW(55296),'a','b',1,-2)", EvalError::Domain),
        ("Replace(ChrW(55296),'a','b',1,-1,3)", EvalError::Domain),
        (
            "Replace(ChrW(55296),NULL,'b')",
            EvalError::InvalidNullConversion,
        ),
        (
            "Replace(NULL,ChrB(65),'b')",
            EvalError::InvalidNullConversion,
        ),
        (
            "Replace(ChrW(55296),CVErr(1),'b',1,0)",
            EvalError::TypeMismatch,
        ),
        ("Replace(ChrB(65),'','b')", EvalError::UnrepresentableText),
        ("Replace('',ChrB(65),'b')", EvalError::UnrepresentableText),
        (
            "Replace(ChrW(55296),'a',ChrB(65),1,0)",
            EvalError::UnrepresentableText,
        ),
        ("Replace(ChrW(55296),'a',1/0)", EvalError::DivisionByZero),
        ("Replace(ChrB(65),'b','c',1/0)", EvalError::DivisionByZero),
        (
            "Replace(ChrW(55296),'a','b')",
            EvalError::UnrepresentableText,
        ),
        (
            "Replace('😀',ChrW(55357),'')",
            EvalError::UnrepresentableText,
        ),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn retained_api_keeps_empty_states_and_strict_value_api_publication_separate() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    let arguments = [raw(&[0xd800]), raw(&[0xd800]), ExpressionValue::Empty];
    let result = evaluator.apply_retained_replacement(&arguments).unwrap();
    assert!(matches!(result, ByteOperand::Bytes(bytes) if bytes.as_bytes().is_empty()));

    let arguments = [raw(&[0xd800]), raw(&[65]), raw(&[66])];
    assert_eq!(
        evaluator.apply_expression_replacement(&arguments),
        Err(EvalError::UnrepresentableText)
    );
    let arguments = [raw(&[0xd800]), ExpressionValue::Missing, raw(&[66])];
    assert!(matches!(
        evaluator.apply_retained_replacement(&arguments),
        Err(EvalError::NonScalarExpression)
    ));
}

#[test]
fn raw_replacement_charges_buffers_output_and_every_observed_cancellation_checkpoint() {
    let arguments = [raw(&[0xd800, 65, 65, 66]), raw(&[65]), raw(&[0xdfff, 65])];
    let checks = Cell::new(0);
    let observe = || {
        checks.set(checks.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&observe)
        .apply_retained_replacement(&arguments)
        .unwrap();
    for stop in 1..=checks.get() {
        let visits = Cell::new(0);
        let cancel = || {
            visits.set(visits.get() + 1);
            visits.get() >= stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&cancel)
                .apply_retained_replacement(&arguments),
            Err(EvalError::Cancelled)
        ));
    }
    for (bytes, steps) in [(0, 10000), (16, 10000), (65536, 0)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .apply_retained_replacement(&arguments),
            Err(EvalError::Limit(_))
        ));
    }
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    let result = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 1,
            ..Default::default()
        },
    )
    .apply_retained_replacement(&[raw(&[65, 66]), raw(&[]), raw(&[])])
    .unwrap();
    assert!(matches!(result, ByteOperand::Bytes(bytes) if bytes.as_bytes() == [65, 0, 66, 0]));
}

fn units(maximum: usize) -> impl Strategy<Value = Vec<u16>> {
    prop::collection::vec(
        prop::sample::select(vec![0, 32, 65, 66, 0xd800, 0xdc00, 0xd83d, 0xde00, 0xffff]),
        0..=maximum,
    )
}

proptest! {
    #[test]
    fn raw_binary_replacement_agrees_with_independent_nonoverlapping_unit_model(
        source in units(12),
        find in units(3),
        replacement in units(4),
        start in 1usize..16,
        count in -1i32..4,
    ) {
        let suffix = &source[(start - 1).min(source.len())..];
        let mut expected = Vec::new();
        let mut cursor = 0;
        let mut left = count;
        while cursor < suffix.len() {
            if !find.is_empty() && left != 0 && suffix[cursor..].starts_with(&find) {
                expected.extend_from_slice(&replacement);
                cursor += find.len();
                if left > 0 {
                    left -= 1;
                }
            } else {
                expected.push(suffix[cursor]);
                cursor += 1;
            }
        }

        let arguments = [
            raw(&source),
            raw(&find),
            raw(&replacement),
            number(start as i32),
            number(count),
        ];
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_retained_replacement(&arguments).unwrap();
        let ByteOperand::Bytes(bytes) = result else {
            panic!("raw result required");
        };
        let expected = expected.iter().flat_map(|unit| unit.to_le_bytes()).collect::<Vec<_>>();
        prop_assert_eq!(bytes.as_bytes(), expected);
    }
}
