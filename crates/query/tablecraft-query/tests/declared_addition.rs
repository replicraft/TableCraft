use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryLimits, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        ..Default::default()
    }
}

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], options(declared))?
        .rows()
        .to_vec())
}

#[test]
fn declared_mixed_addition_selects_operation_from_both_operand_categories() {
    for sql in ["SELECT '2'+CInt(3)", "SELECT CInt(3)+'2'"] {
        assert_eq!(
            run(sql, false).unwrap(),
            [vec![Value::Double(Double::from_float(5.0))]],
            "default {sql}"
        );
        assert_eq!(
            run(sql, true),
            Err(QueryError::Evaluation(EvalError::TypeMismatch)),
            "declared {sql}"
        );
    }

    for (sql, expected) in [
        ("SELECT '2'+CVar(CInt(3))", "23"),
        ("SELECT CVar(CInt(3))+'2'", "32"),
        ("SELECT 'label'+IIf(True,CInt(3),CInt(4))", "label3"),
    ] {
        assert_eq!(
            run(sql, true).unwrap(),
            [vec![Value::Text(expected.into())]]
        );
    }

    for sql in [
        "SELECT CVar('2')+CInt(3)",
        "SELECT CInt(3)+CVar('2')",
        "SELECT CVar('2')+CVar(CInt(3))",
        "SELECT CVar(CInt(3))+CVar('2')",
    ] {
        assert_eq!(
            run(sql, true).unwrap(),
            [vec![Value::Double(Double::from_float(5.0))]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT ('2'+CVar(3))+1", true).unwrap(),
        [vec![Value::Double(Double::from_float(24.0))]]
    );
    assert_eq!(
        run("SELECT CStr('2'+CVar(3))+CVar(1)", true).unwrap(),
        [vec![Value::Text("231".into())]]
    );
    assert_eq!(
        run("SELECT '2'+'3'", true).unwrap(),
        [vec![Value::Text("23".into())]]
    );
}

#[test]
fn unknown_categories_are_refused_only_when_they_can_change_the_family() {
    for sql in [
        "SELECT '2'+First(CInt(3))",
        "SELECT First('2')+CInt(3)",
        "SELECT First('2')+CVar(CInt(3))",
        "SELECT '2'+Q.N FROM (SELECT CInt(3) AS N UNION ALL SELECT CInt(4) WHERE False) AS Q",
    ] {
        assert_eq!(
            run(sql, true),
            Err(QueryError::ArithmeticDeclaration),
            "{sql}"
        );
    }
    for sql in [
        "SELECT CVar('2')+First(CInt(3))",
        "SELECT First(CInt(3))+CVar('2')",
    ] {
        assert_eq!(
            run(sql, true).unwrap(),
            [vec![Value::Double(Double::from_float(5.0))]],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT First('2')+'3'", true).unwrap(),
        [vec![Value::Text("23".into())]]
    );
    assert_eq!(
        run("SELECT NULL+First(CInt(3))", true).unwrap(),
        [vec![Value::Null]]
    );
}

#[test]
fn mixed_result_metadata_is_conservative_across_plan_reuse_and_composition() {
    let source = MemorySource::default();
    for sql in [
        "SELECT '2'+CVar(CInt(3)) AS N",
        "SELECT Q.N FROM (SELECT '2'+CVar(CInt(3)) AS N) AS Q",
        "SELECT '2'+CVar(CInt(3)) AS N UNION ALL SELECT 'tail'",
        "SELECT '2'+CVar(CDec(3)) AS N",
        "SELECT CVar(CCur(3))+'2' AS N",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic], "{sql}");
        for declared in [false, true, false] {
            let result = plan.execute(&source, &[], options(declared)).unwrap();
            let output = result.output_types()[0];
            if declared {
                assert!(matches!(
                    output,
                    OutputType::Scalar { data_type, .. }
                        if data_type.scalar_type() == ScalarType::Text
                ));
            } else {
                assert_eq!(output, OutputType::Dynamic);
            }
            let first = &result.rows()[0][0];
            assert_eq!(matches!(first, Value::Text(_)), declared, "{sql}");
        }
    }
    let parsed = parse_query("SELECT 'a'+'b', 'a' & CInt(3)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for output in plan.output_types().unwrap() {
        assert!(
            matches!(output, OutputType::Scalar { data_type, .. } if data_type.scalar_type() == ScalarType::Text)
        );
    }
}

struct Named {
    source: MemorySource,
    query: Query,
    scans: Cell<usize>,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("V").then_some(&self.query))
    }

    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.source.columns(name)
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.source.scan(name, visitor)
    }
}

#[test]
fn named_sources_parameters_and_correlated_subqueries_keep_declared_categories() {
    let source = Named {
        source: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), ScalarType::Int16, false).unwrap()],
                vec![vec![Value::Int16(3)]],
            )
            .unwrap(),
        ])
        .unwrap(),
        query: parse_query("SELECT N, CVar(N) AS V FROM T", Default::default())
            .unwrap()
            .into_query(),
        scans: Cell::new(0),
    };
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT P+V, (SELECT P+Q.V) FROM V AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(&source, &[("P", Value::Text("2".into()))], options(true))
            .unwrap()
            .rows(),
        [vec![Value::Text("23".into()), Value::Text("23".into())]]
    );
    assert_eq!(source.scans.get(), 1);

    let parsed = parse_query("SELECT '2'+N FROM V", Default::default()).unwrap();
    assert!(matches!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options(true)),
        Err(QueryError::Evaluation(EvalError::TypeMismatch))
    ));
}

#[test]
fn eager_errors_nulls_explicit_formatting_and_lossless_strings_keep_their_contracts() {
    assert_eq!(
        run("SELECT 'x'+CVar(NULL), NULL+'x'", true).unwrap(),
        [vec![Value::Null, Value::Null]]
    );
    for sql in [
        "SELECT 'x'+CVar(1/0)",
        "SELECT NULL+('x'+CVar(1/0))",
        "SELECT IIf(True,'x'+CVar(CInt(3)),1/0)",
    ] {
        assert_eq!(
            run(sql, true),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    assert_eq!(
        run("SELECT Len(StrReverse(ChrW(55296))+CVar(CInt(3)))", true).unwrap(),
        [vec![Value::Int32(2)]]
    );

    let source = MemorySource::default();
    let parsed = parse_query("SELECT 'x'+CVar(CInt(3))", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        ),
        Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
    ));
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                max_text_bytes: Some(1),
                ..options(true)
            }
        ),
        Err(QueryError::Evaluation(EvalError::TextLimit))
    ));
    assert!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    steps: 1,
                    ..Default::default()
                },
                ..options(true)
            }
        )
        .is_err()
    );
    let cancelled = || true;
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..options(true)
            }
        ),
        Err(QueryError::Cancelled)
    ));
}
