use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

fn run(sql: &str, source: &dyn Source, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn casts_variants_nested_results_and_all_integral_operators_keep_declarations() {
    let source = MemorySource::default();
    for sql in [
        "SELECT CByte(255)+CByte(1)",
        "SELECT CInt(32767)+CInt(1)",
        "SELECT CLng(2147483647)+CLng(1)",
        "SELECT -CInt(-32768)",
        "SELECT CInt(-32768)\\CInt(-1)",
        "SELECT Q.N+CInt(1) FROM (SELECT CInt(32767) AS N) AS Q",
        "SELECT (SELECT Q.N+CInt(1)) FROM (SELECT CInt(32767) AS N) AS Q",
    ] {
        assert!(run(sql, &source, false).is_ok(), "{sql}");
        assert_eq!(
            run(sql, &source, true),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }
    for sql in [
        "SELECT CVar(CInt(32767))+CInt(1)",
        "SELECT CInt(32767)+CVar(CInt(1))",
        "SELECT -CVar(CInt(-32768))",
        "SELECT Q.N+CInt(1) FROM (SELECT CVar(CInt(32767)) AS N) AS Q",
        "SELECT (SELECT Q.N+CInt(1)) FROM (SELECT CVar(CInt(32767)) AS N) AS Q",
        "SELECT IIf(True,CInt(32767),CInt(0))+CInt(1)",
        "SELECT Nz(CInt(32767),CInt(0))+CInt(1)",
        "SELECT Choose(1,CInt(32767))+CInt(1)",
        "SELECT Switch(True,CInt(32767))+CInt(1)",
    ] {
        assert_eq!(
            run(sql, &source, true).unwrap(),
            [vec![Value::Int32(32768)]],
            "{sql}"
        );
    }
    assert_eq!(
        run(
            "SELECT CInt(7)-CInt(2),CInt(3)*CInt(2),CInt(7)\\CInt(2),CInt(7) Mod CInt(2)",
            &source,
            true
        )
        .unwrap(),
        [vec![
            Value::Int16(5),
            Value::Int16(6),
            Value::Int16(3),
            Value::Int16(1)
        ]]
    );
}

#[test]
fn union_and_aggregate_unknowns_are_not_guessed_from_selected_or_present_rows() {
    let source = MemorySource::default();
    for sql in [
        "SELECT Q.N+CInt(1) FROM (SELECT CInt(2) AS N UNION ALL SELECT CInt(3)) AS Q",
        "SELECT Q.N+CInt(1) FROM (SELECT CInt(2) AS N UNION ALL SELECT CVar(CInt(3)) WHERE False) AS Q",
        "SELECT First(CInt(2))+CInt(1)",
        "SELECT Abs(CInt(1))+CInt(1)",
    ] {
        assert!(run(sql, &source, false).is_ok());
        assert_eq!(
            run(sql, &source, true),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT CVar(First(CInt(2)))+CInt(1)", &source, true).unwrap(),
        [vec![Value::Int16(3)]]
    );
    assert_eq!(
        run("SELECT NULL+First(CInt(2))", &source, true).unwrap(),
        [vec![Value::Null]]
    );
}

struct Named {
    table: MemorySource,
    query: Query,
    scans: Cell<usize>,
}
impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Named").then_some(&self.query))
    }
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.table.columns(name)
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.table.scan(name, visitor)
    }
}

#[test]
fn source_fields_named_queries_parameters_and_option_reuse_do_not_infer_from_scans() {
    let source = Named {
        table: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), ScalarType::Int16, false).unwrap()],
                vec![vec![Value::Int16(32767)]],
            )
            .unwrap(),
        ])
        .unwrap(),
        query: parse_query("SELECT CVar(N) AS V,N AS S FROM T", Default::default())
            .unwrap()
            .query()
            .clone(),
        scans: Cell::new(0),
    };
    let parsed = parse_query("SELECT V+CInt(1) FROM Named", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(32768)]]
    );
    assert_eq!(
        run("SELECT S+CInt(1) FROM Named", &source, true),
        Err(QueryError::Evaluation(EvalError::Overflow))
    );
    let parsed = parse_query("PARAMETERS P SHORT; SELECT P+CInt(1)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let parameters = [("P", Value::Int16(32767))];
    for declared in [false, true, false] {
        let result = plan.execute(
            &source,
            &parameters,
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        );
        assert_eq!(result.is_ok(), !declared);
    }
}

#[test]
fn eager_errors_cancellation_and_budgets_remain_visible() {
    let source = MemorySource::default();
    assert_eq!(
        run("SELECT CVar(NULL)+(1/0)", &source, true),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    let parsed = parse_query("SELECT CVar(CInt(32767))+CInt(1)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                cancelled: Some(&|| true),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
}

#[test]
fn reviewed_function_declarations_are_distinct_from_equal_result_carriers() {
    let source = MemorySource::default();
    for expression in ["VarType(1)", "Asc('A')", "AscW('A')", "AscB('A')"] {
        let sql = format!("SELECT {expression}+CInt(32767)");
        assert_eq!(
            run(&sql, &source, true),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }
    for expression in ["Len('a')", "LenB('a')"] {
        let sql = format!("SELECT {expression}+CLng(2147483647)");
        assert_eq!(
            run(&sql, &source, true),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }
    for (expression, value) in [
        ("Sgn(1)", 32768),
        ("StrComp('b','a',0)", 32768),
        ("Year(#2026-01-01#)", 34793),
    ] {
        let sql = format!("SELECT Q.N+CInt(32767) FROM (SELECT {expression} AS N) AS Q");
        assert_eq!(
            run(&sql, &source, true).unwrap(),
            [vec![Value::Int32(value)]],
            "{sql}"
        );
    }
    for expression in ["InStr(1,'a','a',0)", "InStrB(1,'a','a',0)"] {
        let sql = format!("SELECT {expression}+CLng(2147483647)");
        assert_eq!(
            run(&sql, &source, true).unwrap(),
            [vec![Value::Double(Double::from_float(2147483648.0))]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT Len(NULL)+CInt(1)", &source, true).unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT Sgn(1/0)+CInt(1)", &source, true),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn date_search_and_color_functions_preserve_reviewed_declarations() {
    let source = MemorySource::default();
    for expression in [
        "InStrRev('abc','a',-1,0)",
        "RGB(1,0,0)",
        "RGB(CVar(1),0,0)",
        "QBColor(1)",
    ] {
        let sql = format!("SELECT Q.N+CLng(2147483647) FROM (SELECT {expression} AS N) AS Q");
        assert!(run(&sql, &source, false).is_ok(), "{sql}");
        assert_eq!(
            run(&sql, &source, true),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }
    let date = "#2026-10-22 15:16:17#";
    for (function, value) in [
        ("Month", 10),
        ("Day", 22),
        ("Hour", 15),
        ("Minute", 16),
        ("Second", 17),
        ("Weekday", 5),
    ] {
        let sql = format!("SELECT Q.N+CInt(32767) FROM (SELECT {function}({date}) AS N) AS Q");
        assert_eq!(
            run(&sql, &source, true).unwrap(),
            [vec![Value::Int32(32767 + value)]],
            "{sql}"
        );
        let sql = format!("SELECT CInt({function}({date}))+CInt(32767)");
        assert_eq!(
            run(&sql, &source, true),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
    }
    assert_eq!(
        run(
            "SELECT DatePart('yyyy',#2026-10-22#)+CInt(32767)",
            &source,
            true
        )
        .unwrap(),
        [vec![Value::Int32(34793)]]
    );
    assert_eq!(
        run(
            "SELECT DateDiff('d',#2026-10-21#,#2026-10-22#)+CLng(2147483647)",
            &source,
            true
        )
        .unwrap(),
        [vec![Value::Double(Double::from_float(2147483648.0))]]
    );
    assert_eq!(
        run("SELECT DatePart('yyyy',NULL)+CInt(1)", &source, true).unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT RGB(NULL,1/0,0)+CLng(1)", &source, true),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn rounded_operand_division_retains_original_scalar_and_variant_categories() {
    let source = MemorySource::default();
    for (left, right) in [
        ("CInt(-32768)", "CDbl(-1)"),
        ("CDbl(-2147483648)", "CInt(-1)"),
        ("CSng(-2147483648)", "CInt(-1)"),
        ("CCur(-2147483648)", "CInt(-1)"),
        ("CInt(-32768)", "#1899-12-29#"),
    ] {
        let sql = format!("SELECT {left}\\{right}");
        assert!(run(&sql, &source, false).is_ok(), "{sql}");
        assert_eq!(
            run(&sql, &source, true),
            Err(QueryError::Evaluation(EvalError::Overflow)),
            "{sql}"
        );
        let sql = format!("SELECT CVar({left})\\{right}");
        assert!(run(&sql, &source, true).is_ok(), "{sql}");
        let sql = format!("SELECT {left}\\CVar({right})");
        assert!(run(&sql, &source, true).is_ok(), "{sql}");
    }
    let query = parse_query(
        "PARAMETERS D TEXT; SELECT CInt(7)\\D,CInt(7) Mod D",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (format, text, quotient, remainder) in [
        (
            NumberFormat::EnUs,
            "2.50000000000000000000000000000000001",
            2,
            1,
        ),
        (NumberFormat::DeDe, "2,5", 3, 1),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("D", Value::Text(text.into()))],
                ExecutionOptions {
                    declared_integral: true,
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![Value::Int16(quotient), Value::Int16(remainder)]]
        );
    }
    assert_eq!(
        run("SELECT CInt(7)\\CDbl(0.49)", &source, true),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT NULL\\'bad'", &source, true).unwrap(),
        [vec![Value::Null]]
    );
}
