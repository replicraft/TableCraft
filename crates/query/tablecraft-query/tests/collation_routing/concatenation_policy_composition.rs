//! Retained complete String policies compose without erasing real contributors.
use super::support::*;
use tablecraft_expressions::functions::conversions::text::NumberFormat;

fn execute(
    sql: &str,
    source: &Fixture,
    resolver: &Resolver,
    declared: bool,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source)?;
    plan.execute(
        source,
        &[],
        ExecutionOptions {
            number_format: Some(NumberFormat::Invariant),
            collation: Some(&DEFAULT),
            collation_resolver: Some(resolver),
            declared_integral: declared,
            ..Default::default()
        },
    )
    .map(|result| result.rows().to_vec())
}

#[test]
fn grouped_distinct_and_extrema_keys_use_actual_concatenated_string_contributors() {
    let source = Fixture::new();
    let key = "A & IIf(True,0,B)";
    for declared in [false, true] {
        let resolver = Resolver::new();
        let grouped = format!("SELECT {key} AS K,Count(*) FROM T GROUP BY {key} ORDER BY {key}");
        assert_eq!(
            execute(&grouped, &source, &resolver, declared).unwrap(),
            [
                vec![Value::Text("a0".into()), Value::Int64(2)],
                vec![Value::Text("b0".into()), Value::Int64(2)],
            ]
        );

        let distinct = format!("SELECT DISTINCT {key} AS K FROM T ORDER BY K");
        assert_eq!(
            execute(&distinct, &source, &resolver, declared).unwrap(),
            texts(&["a0", "b0"])
        );

        let extrema = format!("SELECT Min(K),Max(K) FROM (SELECT {key} AS K FROM T) Q");
        assert_eq!(
            execute(&extrema, &source, &resolver, declared).unwrap(),
            [vec![Value::Text("a0".into()), Value::Text("b0".into())]]
        );
        assert!(resolver.hits.borrow().contains(&1));
        assert!(!resolver.hits.borrow().contains(&2));
    }
}

#[test]
fn complete_default_and_ordinary_field_rows_keep_the_actual_union_policy() {
    let mut source = Fixture::new();
    for row in &mut source.rows {
        row[1] = Value::Text("00".into());
    }
    let resolver = Resolver::new();
    let sql = concat!(
        "SELECT Count(*) FROM (",
        "SELECT CStr(0) & IIf(True,0,B) AS S FROM T WHERE Id=1 ",
        "UNION ALL SELECT A FROM T WHERE Id=1) Q WHERE S='00'",
    );
    assert_eq!(
        execute(sql, &source, &resolver, false).unwrap(),
        [vec![Value::Int64(2)]]
    );
    assert!(resolver.hits.borrow().contains(&1));
    assert!(!resolver.hits.borrow().contains(&2));

    let sql = concat!(
        "SELECT S,Count(*) FROM (",
        "SELECT CStr(0) & IIf(True,0,B) AS S FROM T WHERE Id=1 ",
        "UNION ALL SELECT A FROM T WHERE Id=1) Q GROUP BY S",
    );
    assert_eq!(
        execute(sql, &source, &resolver, false).unwrap(),
        [vec![Value::Text("00".into()), Value::Int64(2)]]
    );
}

#[test]
fn complete_cells_do_not_hide_actual_text_conflicts_in_selectors_or_sets() {
    let source = Fixture::new();
    for sql in [
        "SELECT IIf(True,A & IIf(True,0,B),B)='a0' FROM T",
        concat!(
            "SELECT S='a0' FROM (",
            "SELECT A & IIf(True,0,B) AS S FROM T WHERE Id=1 ",
            "UNION ALL SELECT B FROM T WHERE Id=1) Q",
        ),
        concat!(
            "SELECT Count(*) FROM (",
            "SELECT A & IIf(True,0,B) AS S FROM T WHERE Id=1 ",
            "UNION ALL SELECT B FROM T WHERE Id=1) Q GROUP BY S",
        ),
        concat!(
            "SELECT S FROM (",
            "SELECT A & IIf(True,0,B) AS S FROM T WHERE Id<=2 ",
            "UNION ALL SELECT B FROM T WHERE False) Q ORDER BY S",
        ),
    ] {
        let resolver = Resolver::new();
        assert_eq!(
            execute(sql, &source, &resolver, false).err(),
            Some(QueryError::CollationConflict),
            "{sql}"
        );
    }
}

#[test]
fn pivot_headings_and_named_aggregate_cells_keep_complete_string_policies() {
    let mut source = Fixture::new();
    let resolver = Resolver::new();
    let pivot = concat!(
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 ",
        "PIVOT A & IIf(True,0,B)",
    );
    let syntax = parse_query(pivot, Default::default()).unwrap();
    let result = prepare(syntax.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                collation: Some(&DEFAULT),
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.columns(), ["G", "a0", "b0"]);
    assert_eq!(
        result.rows(),
        [vec![Value::Int64(1), Value::Int64(2), Value::Int64(2)]]
    );

    let named = concat!(
        "TRANSFORM Min(A & IIf(True,0,B)) SELECT 1 AS G FROM T ",
        "GROUP BY 1 PIVOT Id Mod 2 IN (0,1)",
    );
    source.named = parse_query(named, Default::default()).unwrap().into_query();
    assert_eq!(
        execute(
            "SELECT [0]='A0',[1]='A0' FROM Named",
            &source,
            &resolver,
            false
        )
        .unwrap(),
        [vec![Value::Boolean(true), Value::Boolean(true)]]
    );
    assert!(resolver.hits.borrow().contains(&1));
    assert!(!resolver.hits.borrow().contains(&2));
}
