use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn typed_numeric_text_uses_selected_width_without_changing_default_comparisons() {
    for (expression, materialized, declared) in [
        ("CByte(2)='2.5'", false, true),
        ("'3.5'=CInt(4)", false, true),
        ("CLng(-2)='-2.5'", false, true),
        (
            "CLngLng('9007199254740993')='9007199254740992'",
            true,
            false,
        ),
        ("CLngLng('9007199254740993')='9007199254740993'", true, true),
        ("CInt(2)=CVar('2.5')", false, true),
        ("CInt(2)=First('2.5')", false, true),
    ] {
        let sql = format!("SELECT {expression}");
        for (mode, expected) in [(false, materialized), (true, declared)] {
            assert_eq!(
                run(&sql, mode).unwrap(),
                [vec![Value::Boolean(expected)]],
                "{sql}, declared={mode}"
            );
        }
    }
}

#[test]
fn every_relational_operator_and_composed_predicate_uses_the_same_conversion() {
    for expression in [
        "CInt(2)='2.5'",
        "NOT (CInt(2)<>'2.5')",
        "CInt(2)<='2.5'",
        "CInt(2)>='2.5'",
        "NOT (CInt(2)<'2.5')",
        "NOT (CInt(2)>'2.5')",
        "CInt(2) BETWEEN '1.5' AND '2.5'",
        "CInt(2) IN ('2.5')",
        "CInt(2)=ANY(SELECT '2.5')",
        "CInt(2)=ALL(SELECT '2.5')",
        "(SELECT CInt(2))='2.5'",
        "N='2.5' FROM (SELECT CInt(2) AS N) Q",
        "N='2.5' FROM (SELECT CInt(2) AS N) Q WHERE N='2.5'",
        "N='2.5' FROM (SELECT CInt(2) AS N) Q GROUP BY N HAVING N='2.5'",
    ] {
        let sql = format!("SELECT {expression}");
        assert_eq!(
            run(&sql, true).unwrap(),
            [vec![Value::Boolean(true)]],
            "{sql}"
        );
    }
}

#[test]
fn errors_nulls_and_unknown_numeric_provenance_retain_their_boundaries() {
    for (expression, expected) in [
        (
            "CByte(255)='255.5'",
            QueryError::Evaluation(EvalError::Overflow),
        ),
        (
            "CInt(0)='32768'",
            QueryError::Evaluation(EvalError::Overflow),
        ),
        (
            "CLng(0)='2147483648'",
            QueryError::Evaluation(EvalError::Overflow),
        ),
        (
            "CInt(0)='bad'",
            QueryError::Evaluation(EvalError::TypeMismatch),
        ),
        (
            "CInt(0)=CStr(1/0)",
            QueryError::Evaluation(EvalError::DivisionByZero),
        ),
        ("First(CInt(2))='2.5'", QueryError::ArithmeticDeclaration),
        (
            "CInt(-32768)\\(CInt(2)=First('2.5'))",
            QueryError::ArithmeticDeclaration,
        ),
    ] {
        assert_eq!(
            run(&format!("SELECT {expression}"), true),
            Err(expected),
            "{expression}"
        );
    }
    assert_eq!(
        run("SELECT CInt(2)=NULL", true).unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT CVar(CInt(2))=CVar('2.5')", true).unwrap(),
        [vec![Value::Boolean(false)]]
    );
}

#[test]
fn prepared_parameter_reuse_converts_current_text_without_changing_declarations() {
    let source = MemorySource::default();
    let syntax = parse_query("PARAMETERS P TEXT; SELECT CInt(2)=P", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (text, expected) in [("2.5", true), ("3.5", false), ("1.5", true)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(text.into()))],
                ExecutionOptions {
                    declared_integral: true,
                    number_format: Some(NumberFormat::Invariant),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Boolean(expected)]]);
    }
}
