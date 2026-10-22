use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, OutputType, PreparedQuery, QueryError, Source, execute_with_schema_discovery,
    prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

fn columns() -> Vec<Column> {
    vec![
        Column::new("N".into(), ScalarType::Int16, false).unwrap(),
        Column::with_type("Label".into(), TypeDescriptor::text(3).unwrap(), false).unwrap(),
    ]
}
fn syntax(sql: &str) -> Query {
    parse_query(sql, ParseLimits::default())
        .unwrap()
        .into_query()
}
fn scalar(data_type: ScalarType, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(data_type),
        nullable,
    }
}
fn text(nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::text(3).unwrap(),
        nullable,
    }
}
struct MetadataOnly {
    columns: Vec<Column>,
    view: Query,
}
impl MetadataOnly {
    fn new() -> Self {
        Self {
            columns: columns(),
            view: syntax("SELECT Label, N FROM T"),
        }
    }
    fn prepare(&self, sql: &str) -> PreparedQuery {
        prepare(&syntax(sql), self).unwrap()
    }
}
impl Source for MetadataOnly {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("V").then_some(&self.view))
    }
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("type metadata must not scan records")
    }
}

#[test]
fn fields_constraints_aliases_parameters_and_dependencies_are_metadata_only() {
    let source = MetadataOnly::new();
    for sql in [
        "SELECT Label AS Renamed, N FROM T",
        "SELECT * FROM V",
        "SELECT D.Label, D.N FROM (SELECT N, Label FROM T) AS D",
    ] {
        let plan = source.prepare(sql);
        assert_eq!(
            plan.output_types().unwrap(),
            [text(false), scalar(ScalarType::Int16, false)]
        );
    }
    let plan = source.prepare("PARAMETERS P TEXT(2); SELECT P, N FROM T");
    assert_eq!(
        plan.output_types().unwrap(),
        [
            OutputType::Scalar {
                data_type: TypeDescriptor::text(2).unwrap(),
                nullable: true
            },
            scalar(ScalarType::Int16, false),
        ]
    );
}

#[test]
fn null_padding_nested_joins_and_empty_subqueries_relax_nullability() {
    let source = MetadataOnly::new();
    for (sql, expected) in [
        (
            "SELECT A.N, B.Label FROM T AS A LEFT JOIN T AS B ON A.N=B.N",
            [scalar(ScalarType::Int16, false), text(true)],
        ),
        (
            "SELECT A.N, B.Label FROM T AS A RIGHT JOIN T AS B ON A.N=B.N",
            [scalar(ScalarType::Int16, true), text(false)],
        ),
        (
            "SELECT A.N, B.Label FROM T AS A INNER JOIN T AS B ON A.N=B.N",
            [scalar(ScalarType::Int16, false), text(false)],
        ),
        (
            "SELECT D.N, D.Label FROM (SELECT B.N, B.Label FROM T AS A LEFT JOIN T AS B ON A.N=B.N) AS D",
            [scalar(ScalarType::Int16, true), text(true)],
        ),
        (
            "SELECT N, (SELECT Label FROM T WHERE N=0) AS Nested FROM T",
            [scalar(ScalarType::Int16, false), text(true)],
        ),
    ] {
        assert_eq!(source.prepare(sql).output_types().unwrap(), expected);
    }
}

#[test]
fn known_conversion_aggregate_and_union_types_do_not_guess_arithmetic_types() {
    let source = MetadataOnly::new();
    let plan = source.prepare("SELECT CByte(N), CInt(N), CLng(N), CLngLng(N), CBool(N), CSng(N), CDbl(N), CCur(N), CDec(N) FROM T");
    let expected = [
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::Boolean,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::Currency,
        ScalarType::Decimal,
    ]
    .map(|kind| scalar(kind, true));
    assert_eq!(plan.output_types().unwrap(), expected);
    assert_eq!(
        source
            .prepare("SELECT Count(*), Min(N), First(Label) FROM T")
            .output_types()
            .unwrap(),
        [
            scalar(ScalarType::Int64, false),
            scalar(ScalarType::Int16, true),
            text(true)
        ]
    );
    assert_eq!(
        source
            .prepare("SELECT N FROM T UNION ALL SELECT CInt(2)")
            .output_types()
            .unwrap(),
        [scalar(ScalarType::Int16, true)]
    );
    assert_eq!(
        source
            .prepare("SELECT N FROM T UNION ALL SELECT CLng(2)")
            .output_types()
            .unwrap(),
        [scalar(ScalarType::Int32, true)]
    );
    assert_eq!(
        source
            .prepare("SELECT N+CInt(1), CVar(N), IIf(True,N,Label) FROM T")
            .output_types()
            .unwrap(),
        [
            OutputType::Dynamic,
            scalar(ScalarType::Int16, false),
            OutputType::Dynamic
        ]
    );
}

#[test]
fn fixed_crosstabs_keep_row_types_and_nullable_cells_without_scanning() {
    let source = MetadataOnly::new();
    let plan =
        source.prepare("TRANSFORM Count(N) SELECT Label FROM T GROUP BY Label PIVOT N IN (1,2)");
    assert_eq!(
        plan.output_types().unwrap(),
        [
            text(false),
            scalar(ScalarType::Int64, true),
            scalar(ScalarType::Int64, true)
        ]
    );
    let dynamic = source.prepare("TRANSFORM Count(N) SELECT Label FROM T GROUP BY Label PIVOT N");
    assert!(dynamic.output_types().is_none());
}

#[test]
fn result_metadata_survives_empty_results_and_matches_materialized_values() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            columns(),
            vec![vec![Value::Int16(1), Value::Text("one".into())]],
        )
        .unwrap(),
    ])
    .unwrap();
    for sql in [
        "SELECT Label, N FROM T",
        "SELECT Label, N FROM T WHERE N=0",
        "SELECT A.N, B.Label FROM T AS A LEFT JOIN T AS B ON A.N=B.N+1",
        "SELECT Count(*), Min(N), First(Label) FROM T WHERE N=0",
        "TRANSFORM Count(N) SELECT Label FROM T GROUP BY Label PIVOT N IN (1,2)",
        "TRANSFORM Count(N) SELECT Label FROM T GROUP BY Label PIVOT N",
    ] {
        let plan = prepare(&syntax(sql), &source).unwrap();
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.output_types().len(), result.columns().len());
        if let Some(expected) = plan.output_types() {
            assert_eq!(result.output_types(), expected);
        }
        for row in result.rows() {
            for (kind, value) in result.output_types().iter().zip(row) {
                if let OutputType::Scalar {
                    data_type,
                    nullable,
                } = kind
                {
                    assert!(*nullable || !matches!(value, Value::Null));
                    assert!(data_type.validate_value(value).is_ok());
                }
            }
        }
    }
}

#[test]
fn predicates_and_compatible_iif_branches_have_conservative_scalar_metadata() {
    let source = MetadataOnly::new();
    let plan = source
        .prepare("SELECT N=1, N BETWEEN 0 AND 2, N IN (1,2), Label LIKE 'x*', IIf(N=1,N,N) FROM T");
    assert_eq!(
        plan.output_types().unwrap(),
        [
            scalar(ScalarType::Boolean, true),
            scalar(ScalarType::Boolean, true),
            scalar(ScalarType::Boolean, true),
            scalar(ScalarType::Boolean, true),
            scalar(ScalarType::Int16, false),
        ]
    );
}

#[test]
fn named_dynamic_crosstab_discovery_preserves_the_bound_result_descriptors() {
    struct Views {
        source: MemorySource,
        view: Query,
    }
    impl Source for Views {
        fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
            Ok(name.eq_ignore_ascii_case("Pivoted").then_some(&self.view))
        }
        fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
            self.source.columns(name)
        }
        fn scan(
            &self,
            name: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            self.source.scan(name, visitor)
        }
    }
    let source = Views {
        source: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                columns(),
                vec![vec![Value::Int16(1), Value::Text("one".into())]],
            )
            .unwrap(),
        ])
        .unwrap(),
        view: syntax("TRANSFORM Count(N) SELECT Label FROM T GROUP BY Label PIVOT N"),
    };
    let result = execute_with_schema_discovery(
        &syntax("SELECT * FROM Pivoted"),
        &source,
        &[],
        ExecutionOptions::default(),
    )
    .unwrap();
    assert_eq!(result.columns(), ["Label", "1"]);
    assert_eq!(
        result.output_types(),
        [text(false), scalar(ScalarType::Int64, true)]
    );
}
