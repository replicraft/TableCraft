use super::*;

#[test]
fn explicit_parameters_validate_names_carriers_and_text_lengths() {
    let source = source();
    let query = parse_query(
        "PARAMETERS Floor BIGINT; SELECT ID FROM Items WHERE Price > Floor ORDER BY ID",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[("floor", Value::Int64(4))],
            ExecutionOptions::default()
        )
        .unwrap()
        .rows(),
        ints(&[&[Some(2)], &[Some(4)]])
    );
    assert_eq!(
        plan.execute(
            &source,
            &[("floor", Value::Int32(4))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Parameter)
    );
    let query = parse_query("PARAMETERS P TEXT(2); SELECT P", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[("p", Value::Text("abc".into()))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
}
#[test]
fn binding_fails_before_scanning_even_when_sources_are_empty() {
    struct Guard {
        columns: Vec<Column>,
        scans: Cell<usize>,
    }
    impl Source for Guard {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            self.scans.set(self.scans.get() + 1);
            Ok(())
        }
    }
    let source = Guard {
        columns: vec![Column::new("ID".into(), ScalarType::Int64, false).unwrap()],
        scans: Cell::new(0),
    };
    for sql in [
        "SELECT Missing FROM T",
        "SELECT Foo(ID) FROM T",
        "SELECT ID FROM T WHERE ID LIKE 'x*'",
        "SELECT ID FROM T AS a, T AS a",
        "SELECT ID FROM T AS a, T AS b",
        "SELECT DISTINCTROW ID FROM T",
        "UPDATE T SET ID=1",
    ] {
        assert!(run(sql, &source).is_err(), "{sql}");
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn sql_integer_parameters_are_long_and_plans_can_be_reused() {
    let source = source();
    let query = parse_query("PARAMETERS P INTEGER; SELECT P + 1", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for n in [3, 7] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Int32(n))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            ints(&[&[Some(i64::from(n) + 1)]])
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Int16(3))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
    assert_eq!(
        plan.execute(
            &source,
            &[("Wrong", Value::Int32(3))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
    let query = parse_query(
        "PARAMETERS A BIGINT, B BIGINT; SELECT A+B",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[("a", Value::Int64(1)), ("A", Value::Int64(2))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
}

#[test]
fn public_syntax_models_are_bounded_independently_of_parser_limits() {
    use tablecraft_sql::{
        query::{ReadBody, Statement},
        select::{BinaryOperator, Expression, Projection},
    };
    let source = source();
    let mut query = parse_query("SELECT 1", ParseLimits::default())
        .unwrap()
        .query()
        .clone();
    let Statement::Read(read) = &mut query.statement else {
        panic!()
    };
    let ReadBody::Select(select) = &mut read.body else {
        panic!()
    };
    let mut expression = Expression::Integer(1);
    for _ in 0..32 {
        expression = Expression::Binary {
            left: Box::new(expression),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::Integer(1)),
        };
    }
    select.projection = vec![Projection::Expression {
        expression,
        alias: None,
    }];
    assert_eq!(
        prepare(&query, &source).err(),
        Some(QueryError::StructureLimit)
    );
}
