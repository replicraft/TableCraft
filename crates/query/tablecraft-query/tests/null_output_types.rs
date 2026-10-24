use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{ParseLimits, parse_query};

fn scalar(kind: ScalarType, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(kind),
        nullable,
    }
}

struct Views {
    scan_allowed: bool,
    source: MemorySource,
    query: tablecraft_sql::query::Query,
}

impl Source for Views {
    fn query(&self, name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
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
        assert!(
            self.scan_allowed,
            "NULL-aware metadata must not scan records"
        );
        self.source.scan(name, visitor)
    }
}

fn source() -> Views {
    Views {
        scan_allowed: true,
        source: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("N".into(), ScalarType::Int16, false).unwrap()],
                vec![vec![Value::Int16(2)]],
            )
            .unwrap(),
        ])
        .unwrap(),
        query: parse_query("SELECT NULL AS Missing,N FROM T", ParseLimits::default())
            .unwrap()
            .into_query(),
    }
}

#[test]
fn null_only_metadata_survives_aliases_named_and_derived_sources() {
    let source = source();
    for sql in [
        "SELECT NULL, -NULL, +NULL, NOT NULL",
        "SELECT Missing,-Missing,+Missing,NOT Missing FROM V",
        "SELECT D.Missing,-D.Missing,+D.Missing,NOT D.Missing FROM (SELECT NULL AS Missing) AS D",
        "SELECT Sum(NULL),Avg(NULL),Min(NULL),Max(NULL)",
        "SELECT First(NULL),Last(NULL),IIf(True,NULL,NULL),CVar(NULL)",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Null; 4], "{sql}");
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Null; 4]], "{sql}");
        assert_eq!(result.output_types(), plan.output_types().unwrap());
    }
}

#[test]
fn null_union_arms_relax_known_descriptors_without_changing_values() {
    let source = source();
    for sql in [
        "SELECT N FROM T UNION ALL SELECT NULL",
        "SELECT NULL AS N UNION ALL SELECT N FROM T",
        "SELECT D.N FROM (SELECT NULL AS N UNION ALL SELECT N FROM T) AS D",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.output_types().unwrap(),
            [scalar(ScalarType::Int16, true)]
        );
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.rows().len(), 2);
        assert!(result.rows().contains(&vec![Value::Null]));
        assert!(result.rows().contains(&vec![Value::Int16(2)]));
    }
}

#[test]
fn selectors_combine_only_value_branches_and_keep_expression_states_dynamic() {
    let source = source();
    for (expression, expected) in [
        ("IIf(N=2,N,NULL)", scalar(ScalarType::Int16, true)),
        ("IIf(N=0,NULL,N)", scalar(ScalarType::Int16, true)),
        ("Switch(N=2,N,False,NULL)", scalar(ScalarType::Int16, true)),
        ("Choose(N-1,N,NULL)", scalar(ScalarType::Int16, true)),
        ("Nz(NULL,N)", scalar(ScalarType::Int16, false)),
        ("Nz(N,'unused')", scalar(ScalarType::Int16, false)),
        ("Nz(IIf(N=2,N,NULL),N)", scalar(ScalarType::Int16, false)),
        ("IIf(N=2,N,CVErr(5))", OutputType::Dynamic),
        ("Switch(N=2,N,True,'unused')", OutputType::Dynamic),
        ("Choose(N-1,N,CLng(2))", OutputType::Dynamic),
    ] {
        let sql = format!("SELECT {expression} FROM T");
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [expected], "{expression}");
        if expected != OutputType::Dynamic {
            let result = plan
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap();
            assert_eq!(result.rows(), [vec![Value::Int16(2)]], "{expression}");
        }
    }
}

#[test]
fn fixed_crosstab_cells_and_empty_outputs_preserve_nullable_descriptors() {
    let source = source();
    for sql in [
        "SELECT IIf(N=2,N,NULL) FROM T WHERE False",
        "SELECT N FROM T WHERE False UNION ALL SELECT NULL WHERE False",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert!(result.rows().is_empty());
        assert_eq!(result.output_types(), [scalar(ScalarType::Int16, true)]);
    }
    let parsed = parse_query(
        "TRANSFORM First(IIf(True,N,NULL)) SELECT N FROM T GROUP BY N PIVOT N IN (2,3)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [
            scalar(ScalarType::Int16, false),
            scalar(ScalarType::Int16, true),
            scalar(ScalarType::Int16, true)
        ]
    );
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Int16(2), Value::Int16(2), Value::Null]]
    );
}

#[test]
fn nullable_composition_binds_using_only_source_declarations() {
    let mut source = source();
    source.scan_allowed = false;
    for sql in [
        "SELECT IIf(True,N,NULL),Switch(True,N,False,NULL),Choose(1,N,NULL),Nz(NULL,N) FROM T",
        "SELECT N FROM T UNION ALL SELECT NULL",
        "SELECT D.N FROM (SELECT NULL AS N UNION ALL SELECT N FROM T) AS D",
        "SELECT Missing,Nz(Missing,N) FROM V",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        prepare(parsed.query(), &source).unwrap();
    }
}

#[test]
fn known_selector_descriptors_do_not_skip_unselected_operand_failures() {
    let source = source();
    for expression in [
        "IIf(True,N,CInt('invalid'))",
        "Switch(True,N,False,CInt('invalid'))",
        "Choose(1,N,CInt('invalid'))",
        "Nz(N,CInt('invalid'))",
    ] {
        let parsed = parse_query(
            &format!("SELECT {expression} FROM T"),
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert!(matches!(
            plan.output_types().unwrap()[0],
            OutputType::Scalar { .. }
        ));
        assert!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .is_err(),
            "{expression}"
        );
    }
}
