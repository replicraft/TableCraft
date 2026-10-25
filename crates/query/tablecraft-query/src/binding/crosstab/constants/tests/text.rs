use super::*;
use tablecraft_expressions::functions::text::TextFunction;

fn call(function: TextFunction, arguments: Vec<Expr>) -> Expr {
    Expr::TextCall(function, arguments)
}

fn number(value: i32) -> Expr {
    Expr::NumericLiteral(Value::Int32(value))
}

#[test]
fn raw_string_observers_do_not_publish_intermediate_surrogates_or_odd_bytes() {
    for (input, expected) in [
        (
            call(
                TextFunction::Len,
                vec![call(TextFunction::ChrW, vec![number(55296)])],
            ),
            Value::Int32(1),
        ),
        (
            call(
                TextFunction::LenB,
                vec![call(TextFunction::ChrB, vec![number(65)])],
            ),
            Value::Int32(1),
        ),
        (
            call(
                TextFunction::AscB,
                vec![call(TextFunction::LeftB, vec![text("ā"), number(1)])],
            ),
            Value::Int16(1),
        ),
        (
            call(
                TextFunction::Len,
                vec![call(TextFunction::Left, vec![text("😀"), number(1)])],
            ),
            Value::Int32(1),
        ),
        (
            call(
                TextFunction::AscW,
                vec![call(TextFunction::Right, vec![text("😀"), number(1)])],
            ),
            Value::Int16(-8704),
        ),
    ] {
        assert_eq!(binder(None, None).constant_heading(&input, 1), Ok(expected));
    }
}

#[test]
fn final_raw_publication_is_strict_and_has_no_unicode_repair() {
    for input in [
        call(TextFunction::ChrW, vec![number(55296)]),
        call(TextFunction::ChrB, vec![number(65)]),
        call(TextFunction::Left, vec![text("😀"), number(1)]),
    ] {
        assert_eq!(
            binder(None, None).constant_heading(&input, 1),
            Err(QueryError::Unsupported)
        );
    }
    assert_eq!(
        binder(None, None).constant_heading(&call(TextFunction::ChrW, vec![number(65)]), 1),
        Ok(Value::Text("A".into()))
    );
}

#[test]
fn closed_selector_invalid_carriers_keep_the_existing_kernel_error() {
    let input = Expr::IIf(
        Box::new(Expr::Literal(Expression::GuidLiteral(
            "00000000-0000-0000-0000-000000000000".into(),
        ))),
        Box::new(text("yes")),
        Box::new(text("no")),
    );
    assert_eq!(
        binder(None, None).constant_heading(&input, 1),
        Err(QueryError::Evaluation(EvalError::TypeMismatch))
    );
}

#[test]
fn text_kernel_work_consumes_shared_remaining_steps_including_failures() {
    let input = call(TextFunction::Len, vec![text(&"x".repeat(100))]);
    let mut small = ResourceBudget::new(100_000, 0, 64);
    assert!(matches!(
        binder(Some(&mut small), None).constant_heading(&input, 1),
        Err(QueryError::Evaluation(EvalError::Limit(error))) if error.resource == Resource::Steps
    ));
    assert_eq!(small.remaining(Resource::Steps), 0);

    let mut ample = ResourceBudget::new(100_000, 0, 1000);
    assert_eq!(
        binder(Some(&mut ample), None).constant_heading(&input, 1),
        Ok(Value::Int32(100))
    );
    assert!(ample.remaining(Resource::Steps) < 900);

    let invalid = call(TextFunction::Left, vec![text("abc"), number(-1)]);
    let before = ample.remaining(Resource::Steps);
    assert_eq!(
        binder(Some(&mut ample), None).constant_heading(&invalid, 1),
        Err(QueryError::Evaluation(EvalError::Domain))
    );
    assert!(ample.remaining(Resource::Steps) < before);
}

#[test]
fn text_folds_keep_shared_allocation_text_and_structural_ceilings() {
    let input = call(TextFunction::StrReverse, vec![text("abcd")]);
    let mut small = ResourceBudget::new(1, 0, 1000);
    assert!(matches!(
        binder(Some(&mut small), None).constant_heading(&input, 1),
        Err(QueryError::Limit(error)) if error.resource == Resource::Bytes
    ));
    let oversized = call(TextFunction::Len, vec![text(&"x".repeat(65_537))]);
    assert_eq!(
        binder(None, None).constant_heading(&oversized, 1),
        Err(QueryError::Evaluation(EvalError::TextLimit))
    );

    let mut deep = text("x");
    for _ in 0..20 {
        deep = call(TextFunction::Trim, vec![deep]);
    }
    assert_eq!(
        binder(None, None).constant_heading(&deep, 1),
        Err(QueryError::StructureLimit)
    );
}

#[test]
fn every_observed_text_fold_checkpoint_can_cancel_without_source_access() {
    let input = call(
        TextFunction::Len,
        vec![call(TextFunction::Trim, vec![text("  Q  ")])],
    );
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    assert_eq!(
        binder(None, Some(&observe)).constant_heading(&input, 1),
        Ok(Value::Int32(1))
    );
    assert!(observed.get() > 5);

    for stop in 1..=observed.get() {
        let callbacks = Cell::new(0);
        let cancelled = || {
            callbacks.set(callbacks.get() + 1);
            callbacks.get() == stop
        };
        assert_eq!(
            binder(None, Some(&cancelled)).constant_heading(&input, 1),
            Err(QueryError::Cancelled),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn no_implicit_formatting_locale_or_new_call_arity_is_admitted() {
    for input in [
        call(TextFunction::Len, vec![number(123)]),
        call(TextFunction::Left, vec![text("abc"), text("1")]),
        call(TextFunction::Left, vec![text("abc")]),
        call(TextFunction::UCase, vec![text("a")]),
        call(TextFunction::Chr, vec![number(65)]),
        call(TextFunction::CStr, vec![number(65)]),
    ] {
        assert_eq!(
            binder(None, None).constant_heading(&input, 1),
            Err(QueryError::Unsupported)
        );
    }
}
