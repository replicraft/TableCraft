use super::*;

#[test]
fn unknown_fixed_width_outputs_execute_without_inventing_declarations() {
    let source = MemorySource::default();
    for (sql, expected) in [
        ("SELECT Count(*)+CInt(1)", 2),
        ("SELECT First(CLngLng(7))-CInt(2)", 5),
        ("SELECT First(CLngLng(7))*CInt(2)", 14),
        ("SELECT First(CLngLng(7))\\CInt(2)", 3),
        ("SELECT -First(CLngLng(7))", -7),
        (
            "SELECT Q.N+CInt(2) FROM (SELECT CLngLng(7) AS N UNION ALL SELECT CLngLng(9) WHERE False) AS Q",
            9,
        ),
    ] {
        assert_eq!(
            run(sql, &source, true).unwrap(),
            [vec![Value::Int64(expected)]],
            "{sql}"
        );
    }

    for (sql, expected) in [
        ("SELECT First(CByte(255))\\CByte(2)", Value::Byte(127)),
        ("SELECT -First(CByte(255))", Value::Int16(-255)),
        ("SELECT -First(CBool(True))", Value::Int16(1)),
    ] {
        assert_eq!(run(sql, &source, true).unwrap(), [vec![expected]], "{sql}");
    }

    // Successful fixed-width arithmetic must not manufacture provenance for a
    // later comparison whose Scalar/Variant result category is still unknown.
    let sql = "SELECT CInt(-32768)\\((Count(*)+CInt(0))=CInt(1))";
    assert_eq!(
        run(sql, &source, true),
        Err(QueryError::ArithmeticDeclaration)
    );
    let sql = "SELECT CInt(-32768)\\((Count(*)+CInt(0))=CVar(CInt(1)))";
    assert_eq!(
        run(sql, &source, true).unwrap(),
        [vec![Value::Int32(32768)]]
    );
}

#[test]
fn unknown_remainder_sources_keep_their_checked_operand_widths() {
    let source = MemorySource::default();
    for (sql, expected) in [
        ("SELECT First(CByte(7)) Mod CByte(2)", Value::Byte(1)),
        ("SELECT First(CInt(-32768)) Mod CInt(-1)", Value::Int16(0)),
        (
            "SELECT First(CLng(-2147483648)) Mod CInt(-1)",
            Value::Int32(0),
        ),
        (
            "SELECT First(CLngLng(-9223372036854775808)) Mod CInt(-1)",
            Value::Int64(0),
        ),
        ("SELECT First(CDbl(7.5)) Mod CInt(2)", Value::Int32(0)),
        ("SELECT First(CInt(7)) Mod CDbl(2.5)", Value::Int16(1)),
    ] {
        assert_eq!(run(sql, &source, true).unwrap(), [vec![expected]], "{sql}");
    }
    for sql in [
        "SELECT First(CInt(-32768))+CInt(-1)",
        "SELECT -First(CInt(-32768))",
        "SELECT First(CInt(-32768))\\CInt(-1)",
        "SELECT (First(CInt(3)) Mod CInt(2))+CInt(32767)",
    ] {
        assert_eq!(
            run(sql, &source, true),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
}

#[test]
fn unknown_categories_do_not_replace_fixed_width_range_zero_or_eager_errors() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT First(CLngLng(9223372036854775807))+CInt(1)",
            EvalError::Overflow,
        ),
        (
            "SELECT -First(CLngLng(-9223372036854775808))",
            EvalError::Overflow,
        ),
        (
            "SELECT First(CLngLng(-9223372036854775808))\\CInt(-1)",
            EvalError::Overflow,
        ),
        (
            "SELECT First(CInt(7)) Mod CInt(0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT First(CInt(7)) Mod CDbl(1e20)", EvalError::Overflow),
        ("SELECT First(CLngLng(7))\\CDbl(1e20)", EvalError::Overflow),
        (
            "SELECT First(CLngLng(7)) Mod (1/0)",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(
            run(sql, &source, true),
            Err(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
}

#[test]
fn unknown_rounded_text_sources_use_request_local_policies_and_exact_lexical_rounding() {
    let source = MemorySource::default();
    for sql in [
        "SELECT First(CLngLng(7))\\'2.5'",
        "SELECT First('7.5') Mod CInt(3)",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        let options = ExecutionOptions {
            declared_integral: true,
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        };
        let result = plan.execute(&source, &[], options).unwrap();
        let expected = if sql.contains('\\') {
            Value::Int64(3)
        } else {
            Value::Int32(2)
        };
        assert_eq!(result.rows(), [vec![expected]]);
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
}
