use super::*;
use crate::evaluation::NoBindings;
use tablecraft_core::{limits::ResourceBudget, values::ExpressionString};
use tablecraft_sql::parse_expression;

#[test]
fn raw_null_predicates_do_not_require_unicode_or_numeric_policies() {
    for raw in ["ChrW(55296)", "ChrB(65)", "CStr(LeftB('😀',1))"] {
        let mut expressions = Vec::new();
        for operator in ["=", "<>", "<", "<=", ">", ">="] {
            expressions.push(format!("{raw} {operator} NULL"));
            expressions.push(format!("NULL {operator} {raw}"));
        }
        expressions.extend([
            format!("{raw} IN (NULL)"),
            format!("{raw} NOT IN (NULL)"),
            format!("NULL IN ({raw})"),
            format!("{raw} BETWEEN NULL AND NULL"),
            format!("{raw} NOT BETWEEN NULL AND NULL"),
        ]);

        for sql in expressions {
            let expression = parse_expression(&sql, Default::default()).unwrap();
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let result =
                Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression);
            assert_eq!(result, Ok(Value::Null), "{sql}");
        }
    }
}

#[test]
fn raw_predicate_state_and_eager_failures_are_not_null_results() {
    for (sql, expected) in [
        ("(ChrW(55296)=NULL) AND (1/0)", EvalError::DivisionByZero),
        ("ChrW(55296) IN (NULL,1/0)", EvalError::DivisionByZero),
        ("ChrB(65) BETWEEN NULL AND 1/0", EvalError::DivisionByZero),
        ("ChrW(65536)=NULL", EvalError::Domain),
        ("ChrW(55296)=CVErr(1)", EvalError::TypeMismatch),
        ("ChrW(55296)='a'", EvalError::UnrepresentableText),
    ] {
        let expression = parse_expression(sql, Default::default()).unwrap();
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result =
            Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression);
        assert_eq!(result, Err(expected), "{sql}");
    }
}

#[test]
fn already_charged_raw_null_comparison_allocates_nothing() {
    let raw = ExpressionValue::StringBytes(ExpressionString::from_owned(vec![65]));
    let null = ExpressionValue::Value(Value::Null);
    let mut budget = ResourceBudget::new(0, 0, 1);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default()).compare_expression_values(
            Comparison::Equal,
            &raw,
            &null
        ),
        Ok(Truth::Unknown)
    );
}
