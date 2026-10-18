use super::*;
use crate::EvalError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, Value},
};
use tablecraft_sql::{
    ParseLimits, parse_select,
    select::{Expression, FieldReference, Projection, UnaryOperator},
};

fn expression(sql: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {sql}"), ParseLimits::default()).unwrap();
    match &parsed.query().projection[0] {
        Projection::Expression { expression, .. } => expression.clone(),
        Projection::Field { field, .. } => Expression::Field(field.clone()),
        Projection::Wildcard(_) => panic!("scalar expression expected"),
    }
}
fn evaluate(sql: &str) -> Result<Value, EvalError> {
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65_536, 0, 1024),
        EvalLimits::default(),
    )
    .evaluate(&expression(sql))
}

#[test]
fn parsed_scalar_expressions_use_access_precedence_and_null_truth() {
    for (sql, result) in [
        ("2 + 3 * 4", Value::Int64(14)),
        ("-2 ^ 2", Value::Double(Double::from_float(-4.0))),
        ("12.6 Mod 5", Value::Int64(3)),
        ("9.5 \\ 2.5", Value::Int64(5)),
        ("TRUE + 2", Value::Int64(1)),
        ("NULL = NULL", Value::Null),
        ("NULL <> 2", Value::Null),
        ("FALSE AND NULL", Value::Boolean(false)),
        ("TRUE OR NULL", Value::Boolean(true)),
        ("NOT NULL", Value::Null),
        ("FALSE IMP NULL", Value::Boolean(true)),
        ("TRUE XOR NULL", Value::Null),
        ("TRUE EQV FALSE", Value::Boolean(false)),
        ("NULL IS NULL", Value::Boolean(true)),
        ("'' IS NOT NULL", Value::Boolean(true)),
        ("'a' & NULL", Value::Text("a".into())),
        ("'a' + NULL", Value::Null),
        ("NULL & NULL", Value::Null),
        ("'a' + 'b'", Value::Text("ab".into())),
    ] {
        assert_eq!(evaluate(sql), Ok(result), "{sql}");
    }
}

#[test]
fn fields_and_parameters_are_supplied_by_the_host_with_qualifiers_intact() {
    struct Row {
        money: Value,
        decimal: Value,
        integer: Value,
        text: Value,
    }
    impl Bindings for Row {
        fn resolve(&self, field: &FieldReference) -> Result<&Value, EvalError> {
            match (
                field.qualifier.as_ref().map(|n| n.as_str()),
                field.name.as_str(),
            ) {
                (Some("r"), "Amount") => Ok(&self.money),
                (None, "Minimum") => Ok(&self.decimal),
                (None, "Count") => Ok(&self.integer),
                (None, "Title") => Ok(&self.text),
                _ => Err(EvalError::UnboundField),
            }
        }
    }
    let row = Row {
        money: Value::Currency(Currency::from_scaled(12_500)),
        decimal: Value::Decimal(Decimal::new(125, 2).unwrap()),
        integer: Value::Int32(3),
        text: Value::Text("title".into()),
    };
    let mut budget = ResourceBudget::new(1024, 0, 100);
    let mut evaluator = Evaluator::new(&row, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.evaluate(&expression("r.Amount = [Minimum]")),
        Ok(Value::Boolean(true))
    );
    assert_eq!(
        evaluator.evaluate(&expression("r.Amount * [Count]")),
        Ok(Value::Currency(Currency::from_scaled(37_500)))
    );
    assert_eq!(
        evaluator.evaluate(&expression("[Title] & '!'")),
        Ok(Value::Text("title!".into()))
    );
    assert_eq!(
        evaluator.evaluate(&expression("Amount")),
        Err(EvalError::UnboundField)
    );
    assert_eq!(evaluate("MissingField"), Err(EvalError::UnboundField));
    let mut budget = ResourceBudget::new(1024, 0, 100);
    assert_eq!(
        Evaluator::new(
            &row,
            &mut budget,
            EvalLimits {
                max_depth: 32,
                max_text_bytes: 3
            }
        )
        .evaluate(&expression("[Title]")),
        Err(EvalError::TextLimit)
    );
}

#[test]
fn evaluation_errors_are_not_hidden_by_boolean_or_null_results() {
    for sql in [
        "FALSE AND (1 / 0 = 0)",
        "TRUE OR (1 / 0 = 0)",
        "NULL + (1 / 0)",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::DivisionByZero), "{sql}");
    }
    assert_eq!(
        evaluate("9223372036854775807 + 1"),
        Err(EvalError::Overflow)
    );
    assert_eq!(evaluate("(-1) ^ 0.5"), Err(EvalError::Domain));
    assert_eq!(evaluate("'2' + 3"), Err(EvalError::TypeMismatch));
    assert_eq!(evaluate("2 AND 1"), Err(EvalError::TypeMismatch));
    assert_eq!(evaluate("'a' = 'a'"), Err(EvalError::CollationRequired));
}

#[test]
fn unimplemented_syntax_never_executes_or_disappears() {
    for sql in [
        "IIf(TRUE, 1, 0)",
        "Mid('abc', , 1)",
        "@@IDENTITY",
        "#1/2/2025#",
        "1 BETWEEN 0 AND 2",
        "1 IN (1, 2)",
        "'a' LIKE 'a*'",
        "(SELECT 1)",
        "Forms!Search.Code",
    ] {
        assert_eq!(
            evaluate(sql),
            Err(EvalError::UnsupportedExpression),
            "{sql}"
        );
    }
    for sql in ["9223372036854775808", "&HFF"] {
        assert_eq!(evaluate(sql), Err(EvalError::UnsupportedLiteral), "{sql}");
    }
    assert_eq!(evaluate("1e9999"), Err(EvalError::Overflow));
    assert_eq!(evaluate("1e-9999"), Err(EvalError::PrecisionLoss));
}

#[test]
fn bytes_and_steps_are_charged_before_work_and_retained_across_calls() {
    let mut budget = ResourceBudget::new(4, 7, 10);
    let error = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .evaluate(&expression("'é' & 'x'"));
    assert!(
        matches!(error, Err(EvalError::Limit(limit)) if limit.resource == Resource::Bytes && limit.remaining == 1)
    );
    assert_eq!(budget.remaining(Resource::Bytes), 1); // Three input bytes charged; result's three bytes rejected.
    assert_eq!(budget.remaining(Resource::Rows), 7);
    assert_eq!(budget.remaining(Resource::Steps), 7);
    let mut budget = ResourceBudget::new(6, 0, 4);
    {
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        assert_eq!(
            evaluator.evaluate(&expression("'é' & 'x'")),
            Ok(Value::Text("éx".into()))
        );
        assert_eq!(
            evaluator.evaluate(&Expression::Integer(1)),
            Ok(Value::Int64(1))
        );
        assert!(
            matches!(evaluator.evaluate(&Expression::Integer(1)), Err(EvalError::Limit(limit)) if limit.resource == Resource::Steps)
        );
    }
    assert_eq!(budget.remaining(Resource::Bytes), 0);
    assert_eq!(budget.remaining(Resource::Steps), 0);
}

#[test]
fn depth_limits_cover_manually_constructed_trees_and_are_clamped() {
    let mut tree = Expression::Integer(1);
    for _ in 0..32 {
        tree = Expression::Unary {
            operator: UnaryOperator::Plus,
            expression: Box::new(tree),
        };
    }
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_depth: usize::MAX,
                max_text_bytes: usize::MAX
            }
        )
        .evaluate(&tree),
        Err(EvalError::DepthLimit)
    );
    let mut budget = ResourceBudget::new(100, 0, 10);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_depth: 1,
                max_text_bytes: 1
            }
        )
        .evaluate(&expression("1 + 2")),
        Err(EvalError::DepthLimit)
    );
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_depth: 1,
                max_text_bytes: 1
            }
        )
        .evaluate(&Expression::Text("é".into())),
        Err(EvalError::TextLimit)
    );
}

#[test]
fn cancellation_is_checked_before_bindings_and_each_subsequent_node() {
    use std::cell::Cell;
    let visits = Cell::new(0);
    let cancelled = || {
        visits.set(visits.get() + 1);
        visits.get() >= 3
    };
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled)
            .evaluate(&expression("1 + 2")),
        Err(EvalError::Cancelled)
    );
    assert_eq!(budget.remaining(Resource::Steps), 98);
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .evaluate(&expression("MissingField")),
        Err(EvalError::Cancelled)
    );
    assert_eq!(budget.remaining(Resource::Steps), 100);
}

#[test]
fn supplied_collation_is_used_and_errors_remain_payload_free() {
    use crate::comparison::Collation;
    struct Ordinal;
    impl Collation for Ordinal {
        fn compare(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, EvalError> {
            Ok(a.cmp(b))
        }
    }
    let mut budget = ResourceBudget::new(100, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_collation(&Ordinal)
            .evaluate(&expression("'a' < 'b'")),
        Ok(Value::Boolean(true))
    );
    let sql = "'private payload' = 1";
    let error = evaluate(sql).unwrap_err();
    assert!(!format!("{error} {error:?}").contains("private payload"));
}

proptest::proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn parsed_numeric_predicates_agree_with_wide_integer_results(a in proptest::prelude::any::<i32>(), b in proptest::prelude::any::<i32>()) {
        let sql = format!("({a} * {b}) >= {a}");
        let expected = i128::from(a) * i128::from(b) >= i128::from(a);
        proptest::prop_assert_eq!(evaluate(&sql), Ok(Value::Boolean(expected)));
    }
}
