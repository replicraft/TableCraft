#[path = "domain/bounds.rs"]
mod bounds;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn data() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Données".into(),
            vec![Column::new("X".into(), ScalarType::Int32, true).unwrap()],
            vec![
                vec![Value::Int32(2)],
                vec![Value::Null],
                vec![Value::Int32(4)],
                vec![Value::Int32(6)],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &dyn Source) -> Result<QueryResult, QueryError> {
    let syntax = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(syntax.query(), source)?.execute(source, &[], ExecutionOptions::default())
}
fn number(value: &Value) -> f64 {
    match value {
        Value::Double(v) => v.to_float(),
        Value::Int64(v) => *v as f64,
        Value::Int32(v) => f64::from(*v),
        _ => panic!("unexpected numeric carrier: {value:?}"),
    }
}
#[test]
fn all_functions_share_filtered_domain_and_existing_numeric_kernels() {
    let source = data();
    let result = run("SELECT DAvg('X','Données'),DCount('*','Données'),DCount('X','Données'),DFirst('X','Données'),DLast('X','Données'),DLookup('X','Données','X>2'),DMin('X','Données'),DMax('X','Données'),DSum('X','Données'),DVar('X','Données'),DVarP('X','Données'),DStDev('X','Données'),DStDevP('X','Données')", &source).unwrap();
    for (actual, expected) in result.rows()[0].iter().zip([
        4.,
        4.,
        3.,
        2.,
        6.,
        4.,
        2.,
        6.,
        12.,
        4.,
        8. / 3.,
        2.,
        (8_f64 / 3.).sqrt(),
    ]) {
        assert!(
            (number(actual) - expected).abs() < 1e-12,
            "{actual:?} != {expected}"
        );
    }
    assert_eq!(result.rows()[0][1], Value::Int64(4));
}
#[test]
fn empty_missing_fields_and_null_rules_are_distinct() {
    let source = data();
    let result = run("SELECT DSum('X','Données','X>9'),DCount('*','Données','X>9'),DLookup('X','Données','X Is Null'),DVarP('X','Données','X=2'),DStDevP('X','Données','X=2'),DCount('*','Données','Absent=1')", &source).unwrap();
    assert_eq!(
        result.rows()[0][..4],
        [Value::Null, Value::Int64(0), Value::Null, Value::Null]
    );
    assert_eq!(result.rows()[0][4], Value::Null);
    assert_eq!(result.rows()[0][5], Value::Null);
    assert_eq!(
        run("SELECT DSum('Absent','Données')", &source).err(),
        Some(QueryError::UnknownName)
    );
    assert!(run("SELECT DCount('*','Absent')", &source).is_err());
    assert_eq!(
        run("SELECT DCount('*','Données',Null)", &source).err(),
        Some(EvalError::InvalidNullConversion.into())
    );
}
#[test]
fn argument_dependencies_parameters_grouping_and_nested_reads_are_retained() {
    let source = data();
    let result = run(
        "SELECT X,DCount('*','Données',IIf(X=2,'X>2',IIf(X=4,'X>4','X>6'))) FROM Données WHERE X Is Not Null ORDER BY X",
        &source,
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(2), Value::Int64(2)],
            vec![Value::Int32(4), Value::Int64(1)],
            vec![Value::Int32(6), Value::Int64(0)]
        ]
    );
    let result = run(
        "SELECT DCount('*','Données','X Is Not Null'),Count(*) FROM Données",
        &source,
    )
    .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int64(3), Value::Int64(4)]]);
    let syntax = parse_query(
        "PARAMETERS P TEXT; SELECT DSum('X',P,'X>2')",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(
        number(
            &plan
                .execute(
                    &source,
                    &[("P", Value::Text("Données".into()))],
                    Default::default()
                )
                .unwrap()
                .rows()[0][0]
        ),
        10.
    );
    assert_eq!(
        run("SELECT (SELECT DCount('*','Données'))", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int64(4)]]
    );
}
#[test]
fn invalid_shapes_injection_and_eager_errors_are_explicit() {
    let source = data();
    for sql in [
        "SELECT DCount('*')",
        "SELECT DCount(*,'Données')",
        "SELECT DCount('*',,'Données')",
        "SELECT Module.DCount('*','Données')",
    ] {
        let syntax = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(prepare(syntax.query(), &source).is_err(), "{sql}");
    }
    for sql in [
        "SELECT DSum('Sum(X)','Données')",
        "SELECT DSum('DCount(\"*\",\"Données\")','Données')",
        "SELECT DCount('*','Données] WHERE True --')",
        "SELECT DCount('*','Données','X=1; SELECT 2')",
        "SELECT DCount('*','Données','Exists(SELECT X FROM Missing)')",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT DSum(Null,'Données',1/0)", &source).err(),
        Some(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT DSum('1/(X-4)','Données')", &source).err(),
        Some(EvalError::DivisionByZero.into())
    );
}

#[test]
fn omitted_final_criteria_is_distinct_from_explicit_null_and_required_slots() {
    let source = data();
    for function in [
        "DAvg", "DCount", "DFirst", "DLast", "DLookup", "DMin", "DMax", "DSum", "DVar", "DVarP",
        "DStDev", "DStDevP",
    ] {
        let omitted = run(&format!("SELECT {function}('X','Données',)"), &source).unwrap();
        let absent = run(&format!("SELECT {function}('X','Données')"), &source).unwrap();
        assert_eq!(omitted.rows(), absent.rows(), "{function}");
        assert_eq!(
            run(&format!("SELECT {function}('X','Données',Null)"), &source).err(),
            Some(EvalError::InvalidNullConversion.into()),
            "{function}"
        );
    }
    for sql in [
        "SELECT DCount(,'Données')",
        "SELECT DCount('*',)",
        "SELECT DCount('*','Données',,)",
    ] {
        let syntax = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(prepare(syntax.query(), &source).is_err(), "{sql}");
    }
}

#[test]
fn criteria_compose_existing_scalar_predicates_and_nested_domain_calls() {
    let source = data();
    for (criteria, expected) in [
        ("X Between 2 And 4", 2),
        ("X In (2,6)", 2),
        ("X Is Null", 1),
        ("Not (X=4)", 2),
        ("X=2 Or X=6", 2),
        ("X Is Null Or X>4", 2),
        ("Abs(X)=4", 1),
        ("X>DMin(\"X\",\"Données\")", 2),
    ] {
        let sql = format!("SELECT DCount('*','Données','{criteria}')");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            [vec![Value::Int64(expected)]],
            "{criteria}"
        );
    }
}
