use super::*;
use crate::evaluation::NoBindings;
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65536, 0, 10000),
        Default::default(),
    )
    .evaluate(&expression)
}

#[test]
fn unicode_observations_keep_unpaired_units_without_publishing_them() {
    for (sql, expected) in [
        ("Len(LeftB('😀',2))", Value::Int32(1)),
        ("AscW(LeftB('😀',2))", Value::Int16(-10179)),
        ("AscW(RightB('😀',2))", Value::Int16(-8704)),
        ("Len(ChrW(55296))", Value::Int32(1)),
        ("LenB(ChrW(55296))", Value::Int32(2)),
        ("AscW(ChrW(55296))", Value::Int16(-10240)),
        ("AscW(CVar(ChrW(-10240)))", Value::Int16(-10240)),
        ("AscB(ChrW(55297))", Value::Int16(1)),
        ("VarType(ChrW(55296))", Value::Int16(8)),
        ("TypeName(ChrW(55296))", Value::Text("String".into())),
        ("InStrB(1,ChrW(55296),ChrW(55296),0)", Value::Int32(1)),
        ("Len(LeftB(Null,2))", Value::Null),
        ("Len(LeftB('x',0))", Value::Int32(0)),
        ("ChrW(65)", Value::Text("A".into())),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }

    for sql in [
        "ChrW(55296)",
        "LeftB('😀',2)",
        "Len(ChrB(65))",
        "AscW(ChrB(65))",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::UnrepresentableText), "{sql}");
    }
    for (sql, expected) in [
        ("AscW(LeftB('x',0))", EvalError::Domain),
        ("Len(ChrW(NULL))", EvalError::InvalidNullConversion),
        ("Len(ChrW(65536))", EvalError::Domain),
        ("Len(ChrW(-32769))", EvalError::Domain),
        ("Len(ChrW(CVErr(1)))", EvalError::TypeMismatch),
        ("AscW(ChrW(1/0))", EvalError::DivisionByZero),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn unicode_intermediates_charge_storage_and_all_observed_work_can_cancel() {
    let expression = parse_expression("AscW(LeftB(ChrW(55296),2))", Default::default()).unwrap();
    let mut bytes = ResourceBudget::new(1, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut bytes, Default::default());
    assert!(matches!(
        evaluator.evaluate(&expression),
        Err(EvalError::Limit(_))
    ));

    let count = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&|| {
            count.set(count.get() + 1);
            false
        })
        .evaluate(&expression)
        .unwrap();
    for stop in 1..=count.get() {
        let current = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                })
                .evaluate(&expression),
            Err(EvalError::Cancelled),
            "checkpoint {stop}"
        );
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn every_utf16_unit_has_the_same_signed_observation_for_both_code_spellings(unit in any::<u16>()) {
        for code in [i32::from(unit), i32::from(unit as i16)] {
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
            let operand = evaluator.apply_byte_string(
                TextFunction::ChrW,
                ByteOperand::Scalar(Value::Int32(code)),
                &[],
            ).unwrap();
            let expected = Value::Int16(unit as i16);
            prop_assert_eq!(evaluator.observe_byte_operand(TextFunction::AscW, operand), Ok(expected));
        }
    }
}
