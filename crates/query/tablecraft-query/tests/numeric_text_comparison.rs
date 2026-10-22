use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable, ScanConstraint},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn scalar_and_relational_predicates_share_contextual_numeric_conversion() {
    let format = Some(NumberFormat::DeDe);
    assert_eq!(
        run(
            "SELECT 2='2,0','2,5'>2,2='1,5',CCur(0.0002)='0,00015',CDec(0.1)=CDbl(0.1)",
            format
        )
        .unwrap(),
        [vec![
            Value::Boolean(true),
            Value::Boolean(true),
            Value::Boolean(false),
            Value::Boolean(true),
            Value::Boolean(true)
        ]]
    );
    assert_eq!(run("SELECT ID FROM (SELECT 1 AS ID UNION ALL SELECT 2 UNION ALL SELECT 3) AS T WHERE ID='2,0'",format).unwrap(),[vec![Value::Int64(2)]]);
    assert_eq!(run("SELECT A.ID FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS A INNER JOIN (SELECT '2,0' AS Code) AS B ON A.ID=B.Code",format).unwrap(),[vec![Value::Int64(2)]]);
    assert_eq!(run("SELECT 2 BETWEEN '3,0' AND '1,0',2 IN ('1,0','2,0'),2 IN (SELECT '2,0'),2=ANY (SELECT '1,0' UNION ALL SELECT '2,0'),2<ALL (SELECT '3,0' UNION ALL SELECT '4,0')",format).unwrap(),[vec![Value::Boolean(true);5]]);
    assert_eq!(
        run("SELECT Eval(\"2='2,0'\")", format).unwrap(),
        [vec![Value::Boolean(true)]]
    );
}

#[test]
fn eager_failures_and_key_equality_are_not_hidden_by_numeric_predicates() {
    assert_eq!(
        run("SELECT 2='2'", None).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
    assert_eq!(
        run("SELECT 2='bad'", Some(NumberFormat::EnUs)).err(),
        Some(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    assert_eq!(
        run("SELECT 'bad'=(1/0)", Some(NumberFormat::EnUs)).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT 2 IN ('2',1/0)", Some(NumberFormat::EnUs)).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT Null='bad'", Some(NumberFormat::EnUs)).unwrap(),
        [vec![Value::Null]]
    );
    assert!(
        run(
            "SELECT DISTINCT X FROM (SELECT 1 AS X UNION ALL SELECT '1') AS Q",
            Some(NumberFormat::EnUs)
        )
        .is_err()
    );
    assert!(
        run(
            "SELECT X,Count(*) FROM (SELECT 1 AS X UNION ALL SELECT '1') AS Q GROUP BY X",
            Some(NumberFormat::EnUs)
        )
        .is_err()
    );
}

#[test]
fn prepared_plans_take_each_executions_number_policy_without_caching_values() {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query("SELECT 1234='1,234'", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (format, expected) in [(NumberFormat::EnUs, true), (NumberFormat::DeDe, false)] {
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Boolean(expected)]]
        );
    }
}

struct Probe {
    data: MemorySource,
    scans: Cell<usize>,
    constrained: Cell<bool>,
}
impl Source for Probe {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(table, visitor)
    }
    fn scan_constrained(
        &self,
        _: &str,
        _: ScanConstraint,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.constrained.set(true);
        Err(QueryError::Source)
    }
}

#[test]
fn mixed_filters_never_extract_partial_hints_or_hide_late_bad_values() {
    let source = Probe {
        data: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![
                    Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                    Column::new("Code".into(), ScalarType::Text, false).unwrap(),
                ],
                vec![
                    vec![Value::Int32(1), Value::Text("2".into())],
                    vec![Value::Int32(2), Value::Text("bad".into())],
                ],
            )
            .unwrap(),
        ])
        .unwrap(),
        scans: Cell::new(0),
        constrained: Cell::new(false),
    };
    let syntax = parse_query("SELECT ID FROM T WHERE ID=1 AND Code=2", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    assert!(!source.constrained.get());
    assert_eq!(source.scans.get(), 1);
    let syntax = parse_query(r#"SELECT DCount('*','T','ID>"1"')"#, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(1)]]
    );
}
