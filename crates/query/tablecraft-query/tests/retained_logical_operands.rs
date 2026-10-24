use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                declared_integral: declared,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn malformed_strings_do_not_preempt_the_other_written_logical_expression() {
    for operator in ["And", "Or", "Xor", "Eqv", "Imp"] {
        for raw in ["ChrW(55296)", "ChrB(65)", "LeftB('😀',2)"] {
            for sql in [
                format!("SELECT {raw} {operator} (1/0)"),
                format!("SELECT First({raw} {operator} (1/0))"),
                format!("SELECT D.S {operator} (1/0) FROM (SELECT {raw} AS S) AS D"),
                format!("SELECT (SELECT D.S {operator} (1/0)) FROM (SELECT {raw} AS S) AS D"),
                format!(
                    "SELECT Count(*) FROM (SELECT {raw} AS S) AS D GROUP BY D.S {operator} (1/0)"
                ),
                format!("SELECT 1 WHERE {raw} {operator} (1/0)"),
                format!("SELECT Count(*) HAVING {raw} {operator} (1/0)"),
            ] {
                for declared in [false, true] {
                    assert_eq!(
                        run(&sql, declared),
                        Err(EvalError::DivisionByZero.into()),
                        "{sql}"
                    );
                }
            }
        }
    }
}

#[test]
fn raw_strings_still_convert_even_when_null_or_zero_could_determine_the_bits() {
    for operator in ["And", "Or", "Xor", "Eqv", "Imp"] {
        for sql in [
            format!("SELECT ChrW(55296) {operator} NULL"),
            format!("SELECT NULL {operator} ChrW(55296)"),
            format!("SELECT CInt(0) {operator} ChrW(55296)"),
            format!("SELECT D.S {operator} NULL FROM (SELECT ChrW(55296) AS S) AS D"),
        ] {
            assert_eq!(
                run(&sql, false),
                Err(EvalError::UnrepresentableText.into()),
                "{sql}"
            );
        }
        let sql = format!("SELECT ChrW(55296) {operator} CVErr(11)");
        assert_eq!(
            run(&sql, false),
            Err(EvalError::TypeMismatch.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT Not ChrW(55296)", false),
        Err(EvalError::UnrepresentableText.into())
    );
}

#[test]
fn retained_numeric_strings_preserve_exact_longlong_rounding_and_scope_composition() {
    let raw = "LeftB('9007199254740993',32)";
    for sql in [
        format!("SELECT {raw} Or CLngLng(0)"),
        format!("SELECT D.S Or CLngLng(0) FROM (SELECT {raw} AS S) AS D"),
        format!("SELECT (SELECT D.S Or CLngLng(0)) FROM (SELECT {raw} AS S) AS D"),
        format!("SELECT D.S Or CLngLng(0) FROM (SELECT {raw} AS S) AS D GROUP BY D.S"),
    ] {
        assert_eq!(
            run(&sql, false).unwrap(),
            vec![vec![Value::Int64(9007199254740993)]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT Not LeftB('2.5',6)", false).unwrap(),
        vec![vec![Value::Int32(-3)]]
    );
    assert_eq!(
        run("SELECT +LeftB('2.5',6),-LeftB('2.5',6)", true).unwrap(),
        vec![vec![
            Value::Double(Double::from_float(2.5)),
            Value::Double(Double::from_float(-2.5)),
        ]]
    );
}

#[test]
fn logical_string_conversion_preserves_the_original_declaration_category() {
    for variant in [false, true] {
        let operand = if variant {
            "LeftB('-2147483648',22)"
        } else {
            "CStr(LeftB('-2147483648',22))"
        };
        for sql in [
            format!("SELECT (Not {operand})+CLng(1)"),
            format!("SELECT D.N+CLng(1) FROM (SELECT Not {operand} AS N) AS D"),
        ] {
            let result = run(&sql, true);
            if variant {
                assert_eq!(
                    result.unwrap(),
                    vec![vec![Value::Double(Double::from_float(2147483648.0))]],
                    "{sql}"
                );
            } else {
                assert_eq!(result, Err(EvalError::Overflow.into()), "{sql}");
            }
        }
    }
}
