use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn euro_calls_retain_group_aggregate_and_correlated_dependencies() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT EuroConvert(A.N,'DEM','EUR'),Count(*) FROM (SELECT 1.2 AS N UNION ALL SELECT 1.2001) AS A GROUP BY EuroConvert(A.N,'DEM','EUR')",
            vec![double(0.61), Value::Int64(2)],
        ),
        (
            "SELECT (SELECT EuroConvert(A.N,'DEM','EUR')) FROM (SELECT 1.2 AS N) AS A",
            vec![double(0.61)],
        ),
        (
            "SELECT EuroConvert(Sum(A.N),'DEM','EUR') FROM (SELECT 1.2 AS N UNION ALL SELECT 1.2) AS A",
            vec![double(1.23)],
        ),
        ("SELECT EuroConvert(1.2,'DEM','EUR',,)", vec![double(0.61)]),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [expected]);
    }
}

#[test]
fn euro_plan_reuse_keeps_currency_and_precision_as_evaluated_operands() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS S TEXT,T TEXT,P SHORT; SELECT EuroConvert(1000,S,T,True,P)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (currency, precision, expected) in [
        ("FRF", 3, Some(152.0 * 1.95583)),
        ("FRF", 17, Some(1000.0 / 6.55957 * 1.95583)),
        ("DEM", 3, Some(1000.0)),
        ("USD", 3, None),
    ] {
        let result = plan.execute(
            &source,
            &[
                ("S", Value::Text(currency.into())),
                ("T", Value::Text("DEM".into())),
                ("P", Value::Int16(precision)),
            ],
            Default::default(),
        );
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![double(expected)]]);
        } else {
            assert_eq!(result.err(), Some(EvalError::Domain.into()));
        }
    }
}
