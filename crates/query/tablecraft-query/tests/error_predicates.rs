use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            [0, 1, 65535].map(|code| vec![Value::Int32(code)]).to_vec(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let query = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn error_relations_flow_through_filters_groups_correlations_and_crosstabs() {
    for sql in [
        "SELECT Count(*) FROM T WHERE CVErr(ID)>=CVErr(1)",
        "SELECT Count(*) FROM (SELECT CVErr(ID) AS Problem FROM T) AS Q WHERE Problem>=CVErr(1)",
        "SELECT Count(*) FROM T AS A WHERE (SELECT CVErr(A.ID))>=CVErr(1)",
    ] {
        assert_eq!(run(sql).unwrap(), [vec![Value::Int64(2)]], "{sql}");
    }
    assert_eq!(
        run("SELECT ID,Count(*) FROM T GROUP BY ID HAVING First(CVErr(ID))=CVErr(1)").unwrap(),
        [vec![Value::Int32(1), Value::Int64(1)]]
    );
    assert_eq!(
        run("TRANSFORM First(CVErr(ID))=CVErr(1) SELECT True AS G FROM T GROUP BY True PIVOT ID IN (0,1,65535)").unwrap(),
        [vec![Value::Boolean(true), Value::Boolean(false), Value::Boolean(true), Value::Boolean(false)]]
    );
}

#[test]
fn lists_ranges_and_quantified_subqueries_compare_error_data_without_storage_coercion() {
    for sql in [
        "SELECT CVErr(1) IN (CVErr(0),CVErr(1)),CVErr(1) NOT IN (CVErr(0))",
        "SELECT CVErr(1) BETWEEN CVErr(2) AND CVErr(0),CVErr(3) NOT BETWEEN CVErr(2) AND CVErr(0)",
        "SELECT CVErr(1) IN (SELECT CVErr(ID) FROM T),CVErr(2) NOT IN (SELECT CVErr(ID) FROM T)",
        "SELECT CVErr(0)<=ALL(SELECT CVErr(ID) FROM T),CVErr(1)=ANY(SELECT CVErr(ID) FROM T)",
        "SELECT CVErr(0)<ALL(SELECT CVErr(ID) FROM T WHERE ID<0),CVErr(0) NOT IN (SELECT CVErr(ID) FROM T WHERE ID<0)",
    ] {
        assert_eq!(run(sql).unwrap(), [vec![Value::Boolean(true); 2]], "{sql}");
    }
    assert_eq!(
        run("SELECT CVErr(0)=ANY(SELECT CVErr(ID) FROM T WHERE ID<0)").unwrap(),
        [vec![Value::Boolean(false)]]
    );
}

#[test]
fn mixed_comparisons_and_logical_operands_are_type_errors_without_catching_failures() {
    for sql in [
        "SELECT CVErr(0)=0",
        "SELECT Null=CVErr(0)",
        "SELECT CVErr(0) IN (CVErr(0),Null)",
        "SELECT CVErr(0) BETWEEN CVErr(0) AND Null",
        "SELECT CVErr(0) IN (SELECT ID FROM T)",
        "SELECT CVErr(0)=ANY(SELECT ID FROM T)",
        "SELECT Not CVErr(0)",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::TypeMismatch.into()),
            "{sql}"
        );
    }
    for operator in ["And", "Or", "Xor", "Eqv", "Imp"] {
        for (left, right) in [
            ("CVErr(0)", "False"),
            ("True", "CVErr(1)"),
            ("Null", "CVErr(0)"),
            ("CVErr(0)", "CVErr(0)"),
        ] {
            let sql = format!("SELECT {left} {operator} {right}");
            assert_eq!(
                run(&sql).err(),
                Some(EvalError::TypeMismatch.into()),
                "{sql}"
            );
        }
        let sql = format!("SELECT CVErr(0) {operator} (1/0)");
        assert_eq!(
            run(&sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT CVErr(0)=(1/0)",
        "SELECT CVErr(0) IN (CVErr(0),1/0)",
        "SELECT CVErr(0) BETWEEN CVErr(0) AND (1/0)",
        "SELECT CVErr(0) IN (SELECT IIf(ID=0,CVErr(0),1/(ID-ID)) FROM T)",
        "SELECT CVErr(0)=ANY(SELECT IIf(ID=0,CVErr(0),1/(ID-ID)) FROM T)",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
}

struct Named {
    data: MemorySource,
    definition: Query,
    scans: Cell<usize>,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Problems").then_some(&self.definition))
    }

    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.data.columns(name)
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(name, visitor)
    }
}

#[test]
fn named_error_predicates_prepare_without_scans_and_share_all_cancellation_checkpoints() {
    let source = Named {
        data: source(),
        definition: parse_query("SELECT CVErr(ID) AS Problem FROM T", Default::default())
            .unwrap()
            .into_query(),
        scans: Cell::new(0),
    };
    let query = parse_query(
        "SELECT Count(*) FROM Problems WHERE Problem>=CVErr(1)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let result = plan
        .execute_expression(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&count),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![ExpressionValue::Value(Value::Int64(2))]]
    );
    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(matches!(
            plan.execute_expression(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
