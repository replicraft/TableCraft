use super::*;
use tablecraft_core::values::ExpressionValue;
use tablecraft_sql::query::Query;

#[test]
fn derived_records_retain_all_output_fields_but_not_unexposed_base_fields() {
    let source = source();
    for (sql, expected) in [
        (
            "SELECT DISTINCTROW D.Shown FROM (SELECT Id,Shown,Hidden FROM A) D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown",
            vec![7, 7, 8],
        ),
        (
            "SELECT DISTINCTROW D.Shown FROM (SELECT Id,Shown FROM A) D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown",
            vec![7, 8],
        ),
        (
            "SELECT DISTINCTROW D.Shown FROM (SELECT Id,Shown,Hidden+1 AS H FROM A) D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown",
            vec![7, 7, 8],
        ),
        (
            "SELECT DISTINCTROW D.Shown FROM (SELECT Id,Shown,Hidden FROM A) D LEFT JOIN B ON D.Id=B.Owner ORDER BY D.Shown",
            vec![7, 7, 8, 9],
        ),
    ] {
        let actual = run(sql, &source).unwrap();
        let expected = expected
            .into_iter()
            .map(|value| vec![Value::Int32(value)])
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}

#[test]
fn derived_record_errors_do_not_become_null_or_disappear_after_deduplication() {
    let source = source();
    let result = run(
        "SELECT DISTINCTROW D.Shown FROM (SELECT Id,Shown,CVErr(5) AS Hidden FROM A) D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown",
        &source,
    );
    assert_eq!(
        result.unwrap(),
        [vec![Value::Int32(7)], vec![Value::Int32(8)]]
    );
    let result = run(
        "SELECT DISTINCTROW D.Shown FROM (SELECT Id,Shown,1/(Hidden-30) AS HiddenValue FROM A) D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown",
        &source,
    );
    assert_eq!(
        result.err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

struct View {
    source: MemorySource,
    definition: Query,
    metadata_only: bool,
}

impl View {
    fn new(sql: &str) -> Self {
        Self {
            source: source(),
            definition: parse_query(sql, ParseLimits::default())
                .unwrap()
                .into_query(),
            metadata_only: false,
        }
    }
}

impl Source for View {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("V").then_some(&self.definition))
    }

    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.source.columns(name)
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(
            !self.metadata_only,
            "record-key binding must not read source rows"
        );
        self.source.scan(name, visitor)
    }
}

#[test]
fn named_union_and_grouped_outputs_are_complete_logical_records() {
    for definition in [
        "SELECT Id,Shown,Hidden FROM A",
        "SELECT Id,Shown,Hidden FROM A UNION ALL SELECT Id,Shown,Hidden FROM A",
        "SELECT Id,Shown,Hidden FROM A UNION SELECT Id,Shown,Hidden FROM A",
        "SELECT D.Id,D.Shown,D.Hidden FROM (SELECT Id,Shown,Hidden FROM A) D",
    ] {
        let source = View::new(definition);
        assert_eq!(
            run(
                "SELECT DISTINCTROW D.Shown FROM V D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown",
                &source
            )
            .unwrap(),
            [
                vec![Value::Int32(7)],
                vec![Value::Int32(7)],
                vec![Value::Int32(8)]
            ]
        );
    }
    let source = View::new("SELECT Id,Sum(Hidden) AS Total FROM A GROUP BY Id");
    assert_eq!(
        run(
            "SELECT DISTINCTROW D.Total FROM V D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Total",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(30)], vec![Value::Int64(50)]]
    );
}

#[test]
fn fixed_and_discovered_crosstab_rows_retain_hidden_cells_in_their_record_keys() {
    let source =
        View::new("TRANSFORM Sum(Hidden) SELECT Id FROM A GROUP BY Id PIVOT Shown IN (7,8,9)");
    let sql = "SELECT DISTINCTROW D.[7] AS V FROM V D LEFT JOIN B ON D.Id=B.Owner ORDER BY V";
    assert_eq!(
        run(sql, &source).unwrap(),
        [vec![Value::Null], vec![Value::Null], vec![Value::Int64(50)]]
    );

    let source = View::new("TRANSFORM Sum(Hidden) SELECT Id FROM A GROUP BY Id PIVOT Shown");
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let result = tablecraft_query::execute_with_schema_discovery(
        parsed.query(),
        &source,
        &[],
        ExecutionOptions::default(),
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![ExpressionValue::Value(Value::Null)],
            vec![ExpressionValue::Value(Value::Null)],
            vec![ExpressionValue::Value(Value::Int64(50))],
        ]
    );
}

#[test]
fn named_record_keys_bind_without_scans_and_ignored_cases_retain_multiplicity() {
    let mut source = View::new("SELECT Id,Shown,Hidden FROM A");
    source.metadata_only = true;
    let parsed = parse_query(
        "SELECT DISTINCTROW D.Shown FROM V D,B WHERE D.Id=B.Owner",
        ParseLimits::default(),
    )
    .unwrap();
    prepare(parsed.query(), &source).unwrap();
    source.metadata_only = false;
    assert_eq!(
        run("SELECT DISTINCTROW D.Shown FROM V D", &source)
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW D.Shown,B.Owner FROM V D INNER JOIN B ON D.Id=B.Owner",
            &source
        )
        .unwrap()
        .len(),
        7
    );
    assert_eq!(run("SELECT DISTINCTROW TOP 1 D.Shown FROM V D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown", &source).unwrap(), [vec![Value::Int32(7)],vec![Value::Int32(7)]]);
}

proptest! {
    #[test]
    fn derived_complete_and_partial_records_match_independent_semijoin_sets(
        values in prop::collection::vec((0_i32..5,0_i32..3,0_i32..5),0..20),
        owners in prop::collection::vec(0_i32..5,0..12),
    ) {
        let rows = values.iter().map(|(id, shown, hidden)| vec![*id,*shown,*hidden]).collect::<Vec<_>>();
        let links = owners.iter().map(|id| vec![*id]).collect::<Vec<_>>();
        let source = MemorySource::new(vec![
            table("A", &["Id","Shown","Hidden"], &rows.iter().map(Vec::as_slice).collect::<Vec<_>>()),
            table("B", &["Owner"], &links.iter().map(Vec::as_slice).collect::<Vec<_>>()),
        ]).unwrap();
        let matched = values.into_iter().filter(|(id,_,_)| owners.contains(id)).collect::<std::collections::BTreeSet<_>>();
        let mut full = matched.iter().map(|(_,shown,_)| *shown).collect::<Vec<_>>();
        full.sort();
        let mut partial = matched.iter().map(|(id,shown,_)| (*id,*shown)).collect::<std::collections::BTreeSet<_>>().into_iter().map(|(_,shown)| shown).collect::<Vec<_>>();
        partial.sort();
        for (columns, expected) in [("Id,Shown,Hidden",full.clone()),("Id,Shown,Hidden+1 AS H",full),("Id,Shown",partial)] {
            let sql = format!("SELECT DISTINCTROW D.Shown FROM (SELECT {columns} FROM A) D INNER JOIN B ON D.Id=B.Owner ORDER BY D.Shown");
            let actual = run(&sql, &source).unwrap();
            let expected = expected.into_iter().map(|value| vec![Value::Int32(value)]).collect::<Vec<_>>();
            prop_assert_eq!(actual, expected);
        }
    }
}
