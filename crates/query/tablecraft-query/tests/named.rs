use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

struct Views {
    tables: MemorySource,
    queries: Vec<(String, Query)>,
    scans: Cell<usize>,
}
#[test]
fn punctuation_prompts_share_compatible_named_values_and_keep_definition_scopes() {
    let source = Views::new(&[
        (
            "A",
            "PARAMETERS [Floor.] LONG; SELECT Id FROM Data WHERE Id>=[Floor.]",
        ),
        (
            "B",
            "PARAMETERS [floor.] INTEGER; SELECT Id FROM Data WHERE Id<=[floor.]",
        ),
        ("Unbound", "SELECT [Floor.] FROM Data"),
    ]);
    assert_eq!(
        source
            .execute(
                "SELECT A.Id FROM A INNER JOIN B ON A.Id=B.Id",
                &[("FLOOR.", Value::Int32(2))]
            )
            .unwrap(),
        [vec![Value::Int32(2)]]
    );
    assert_eq!(
        source
            .execute(
                "PARAMETERS [Floor.] LONG; SELECT * FROM Unbound",
                &[("Floor.", Value::Int32(2))]
            )
            .err(),
        Some(QueryError::UnknownName)
    );
}
fn syntax(sql: &str) -> Query {
    parse_query(sql, ParseLimits::default())
        .unwrap()
        .into_query()
}
impl Views {
    fn new(queries: &[(&str, &str)]) -> Self {
        Self {
            tables: MemorySource::new(vec![
                MemoryTable::new(
                    "Data".into(),
                    ["Id", "Score"]
                        .into_iter()
                        .map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap())
                        .collect(),
                    (1..=3)
                        .map(|id| vec![Value::Int32(id), Value::Int32(id * 10)])
                        .collect(),
                )
                .unwrap(),
            ])
            .unwrap(),
            queries: queries
                .iter()
                .map(|(name, sql)| ((*name).into(), syntax(sql)))
                .collect(),
            scans: Cell::new(0),
        }
    }
    fn execute(
        &self,
        sql: &str,
        parameters: &[(&str, Value)],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        Ok(prepare(&syntax(sql), self)?
            .execute(self, parameters, ExecutionOptions::default())?
            .rows()
            .to_vec())
    }
}
impl Source for Views {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .queries
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, q)| q))
    }
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.tables.columns(name)
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.tables.scan(name, visitor)
    }
}
#[test]
fn nested_named_sources_keep_aliases_wildcards_join_scopes_and_correlations() {
    let source = Views::new(&[
        ("BaseView", "SELECT Id, Score FROM Data"),
        (
            "SecondView",
            "SELECT Id AS Kept, Score FROM BaseView WHERE Score>=20",
        ),
    ]);
    assert_eq!(
        source
            .execute(
                "SELECT SecondView.Kept FROM SecondView ORDER BY Score DESC",
                &[]
            )
            .unwrap(),
        [vec![Value::Int32(3)], vec![Value::Int32(2)]]
    );
    assert_eq!(
        source
            .execute("SELECT V.* FROM SecondView AS V ORDER BY V.Kept", &[])
            .unwrap(),
        [
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(30)]
        ]
    );
    assert_eq!(source.execute("SELECT A.Id, B.Kept FROM BaseView AS A LEFT JOIN SecondView AS B ON A.Id=B.Kept ORDER BY A.Id", &[]).unwrap(), [vec![Value::Int32(1),Value::Null],vec![Value::Int32(2),Value::Int32(2)],vec![Value::Int32(3),Value::Int32(3)]]);
    assert_eq!(source.execute("SELECT D.Id, (SELECT Count(*) FROM BaseView AS B WHERE B.Id<=D.Id) AS N FROM Data AS D ORDER BY D.Id", &[]).unwrap(), [vec![Value::Int32(1),Value::Int64(1)],vec![Value::Int32(2),Value::Int64(2)],vec![Value::Int32(3),Value::Int64(3)]]);
}
#[test]
fn compatible_parameters_merge_by_carrier_while_each_definition_keeps_its_scope() {
    let source = Views::new(&[
        ("A", "PARAMETERS P LONG; SELECT Id FROM Data WHERE Id>=P"),
        ("B", "PARAMETERS p INTEGER; SELECT Id FROM Data WHERE Id<=p"),
        ("Unbound", "SELECT P FROM Data"),
    ]);
    assert_eq!(
        source
            .execute(
                "SELECT A.Id FROM A INNER JOIN B ON A.Id=B.Id",
                &[("P", Value::Int32(2))]
            )
            .unwrap(),
        [vec![Value::Int32(2)]]
    );
    assert_eq!(
        source
            .execute(
                "PARAMETERS P LONG; SELECT P, A.Id FROM A ORDER BY A.Id",
                &[("P", Value::Int32(2))]
            )
            .unwrap(),
        [
            vec![Value::Int32(2), Value::Int32(2)],
            vec![Value::Int32(2), Value::Int32(3)]
        ]
    );
    assert_eq!(
        source.execute("SELECT P FROM A", &[("P", Value::Int32(2))]),
        Err(QueryError::UnknownName)
    );
    assert_eq!(
        source.execute(
            "PARAMETERS P LONG; SELECT * FROM Unbound",
            &[("P", Value::Int32(2))]
        ),
        Err(QueryError::UnknownName)
    );
    assert_eq!(
        source.execute("SELECT * FROM A", &[]),
        Err(QueryError::Parameter)
    );
    let conflict = Views::new(&[
        ("A", "PARAMETERS P LONG; SELECT P"),
        ("B", "PARAMETERS P TEXT; SELECT P"),
    ]);
    assert_eq!(
        conflict.execute("SELECT * FROM A, B", &[]),
        Err(QueryError::Parameter)
    );
    assert_eq!(conflict.scans.get(), 0);
}
#[test]
fn cycles_actions_and_unbounded_chains_fail_before_any_scan() {
    for definitions in [
        vec![("A", "SELECT * FROM a")],
        vec![("A", "SELECT * FROM B"), ("B", "SELECT * FROM A")],
    ] {
        let source = Views::new(&definitions);
        assert_eq!(
            source.execute("SELECT * FROM A", &[]),
            Err(QueryError::DependencyCycle)
        );
        assert_eq!(source.scans.get(), 0);
    }
    let action = Views::new(&[("A", "DELETE FROM Data")]);
    assert_eq!(
        action.execute("SELECT * FROM A", &[]),
        Err(QueryError::Unsupported)
    );
    assert_eq!(action.scans.get(), 0);
    let chain = (0..51)
        .map(|i| (format!("V{i}"), format!("SELECT * FROM V{}", i + 1)))
        .collect::<Vec<_>>();
    let references = chain
        .iter()
        .map(|(n, q)| (n.as_str(), q.as_str()))
        .collect::<Vec<_>>();
    let source = Views::new(&references);
    assert_eq!(
        source.execute("SELECT * FROM V0", &[]),
        Err(QueryError::StructureLimit)
    );
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn bounded_long_chains_and_repeated_acyclic_sources_are_not_mistaken_for_cycles() {
    let mut chain = (0..8)
        .map(|i| (format!("V{i}"), format!("SELECT * FROM V{}", i + 1)))
        .collect::<Vec<_>>();
    chain.push(("V8".into(), "SELECT Id FROM Data".into()));
    let references = chain
        .iter()
        .map(|(name, sql)| (name.as_str(), sql.as_str()))
        .collect::<Vec<_>>();
    let source = Views::new(&references);
    assert_eq!(
        source
            .execute(
                "SELECT A.Id FROM V0 AS A INNER JOIN V0 AS B ON A.Id=B.Id ORDER BY A.Id",
                &[]
            )
            .unwrap(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    let bad = Views::new(&[("Bad", "SELECT 1/0 AS [Value] FROM Data")]);
    assert!(matches!(
        bad.execute("SELECT Count(*) FROM Bad", &[]),
        Err(QueryError::Evaluation(_))
    ));
}

#[test]
fn union_references_bind_only_named_queries_and_keep_final_output_sorting() {
    let source = Views::new(&[
        ("A", "SELECT Id FROM Data WHERE Id=1"),
        ("B", "SELECT Id FROM Data WHERE Id=3"),
    ]);
    assert_eq!(
        source
            .execute("[A] UNION ALL [B] ORDER BY Id DESC", &[])
            .unwrap(),
        [vec![Value::Int32(3)], vec![Value::Int32(1)]]
    );
    assert_eq!(
        source
            .execute("TABLE A UNION ALL TABLE B ORDER BY Id", &[])
            .unwrap(),
        [vec![Value::Int32(1)], vec![Value::Int32(3)]]
    );
    assert_eq!(
        source.execute("[Data] UNION ALL [B]", &[]),
        Err(QueryError::Unsupported)
    );
}
