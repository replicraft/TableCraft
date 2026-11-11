//! No row/function sampling is needed to capture the actual parameter nodes.
mod adversarial;
use super::{ParameterOrigin, prepare_with_parameter_discovery};
use crate::{
    ExecutionOptions, ParameterType, PreparationOptions, QueryError, QueryValue, Source, prepare,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_schema::{Column, ScalarType, SchemaName};
use tablecraft_sql::{ParseLimits, parse_query};

struct Data {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    rows: Vec<Vec<Value>>,
    scans: Cell<usize>,
}
impl Data {
    fn new() -> Self {
        Self {
            columns: vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
            ],
            unavailable: vec![SchemaName::new("Hidden".into()).unwrap()],
            rows: vec![
                vec![Value::Int32(1), Value::Int32(5)],
                vec![Value::Int32(2), Value::Int32(9)],
            ],
            scans: Cell::new(0),
        }
    }
}
impl Source for Data {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if table != "Items" {
            return Err(QueryError::UnknownName);
        }
        Ok(&self.columns)
    }
    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.columns(table)?;
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16_000_000, 100_000, 1_000_000)
}
fn scalar(value: Value) -> QueryValue {
    QueryValue::Scalar(ExpressionValue::Value(value))
}

#[test]
fn explicit_discovery_binds_one_repeated_literal_identity_without_sampling_rows() {
    let data = Data::new();
    let query = parse_query(
        "SELECT Id FROM Items WHERE Score>[Enter minimum:] AND [enter MINIMUM:]>0",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        prepare(query.query(), &data),
        Err(QueryError::UnknownName)
    ));
    let prepared = prepare_with_parameter_discovery(
        query.query(),
        &data,
        PreparationOptions::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let prompts: Vec<_> = prepared.parameters().collect();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "Enter minimum:");
    assert_eq!(prompts[0].data_type, ParameterType::Value);
    assert_eq!(prompts[0].origin, ParameterOrigin::Implicit);
    assert_eq!(data.scans.get(), 0);
    let inputs = [("Enter minimum:", Value::Int32(6))];
    let result = prepared
        .execute_collections(&data, &inputs, ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows(), [vec![scalar(Value::Int32(2))]]);
    assert_eq!(data.scans.get(), 1);
}

#[test]
fn declared_inputs_aliases_and_correlated_outer_fields_keep_existing_resolution() {
    let data = Data::new();
    let query=parse_query("PARAMETERS [minimum] LONG; SELECT i.Id AS Existing, [x.y] AS Prompt FROM Items AS i WHERE i.Score>minimum AND EXISTS (SELECT j.Id FROM Items AS j WHERE j.Id=i.Id) ORDER BY Existing",ParseLimits::default()).unwrap();
    let prepared = prepare_with_parameter_discovery(
        query.query(),
        &data,
        PreparationOptions::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let prompts: Vec<_> = prepared.parameters().collect();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0].name, "minimum");
    assert_eq!(prompts[0].origin, ParameterOrigin::Declared);
    assert_eq!(prompts[1].name, "x.y");
    assert_eq!(prompts[1].origin, ParameterOrigin::Implicit);
    assert_eq!(data.scans.get(), 0);
    let result = prepared
        .execute_collections(
            &data,
            &[
                ("minimum", Value::Int32(6)),
                ("x.y", Value::Text("literal".into())),
            ],
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            scalar(Value::Int32(2)),
            scalar(Value::Text("literal".into()))
        ]]
    );
}

#[test]
fn source_errors_unavailable_ambiguous_and_qualified_names_never_become_prompts() {
    let data = Data::new();
    for (sql, expected) in [
        ("SELECT Misspelled FROM Missing", QueryError::UnknownName),
        ("SELECT Hidden FROM Items", QueryError::Unsupported),
        (
            "SELECT x.Misspelled FROM Items AS x",
            QueryError::UnknownName,
        ),
        (
            "SELECT Id FROM Items AS a,Items AS b",
            QueryError::AmbiguousName,
        ),
        (
            "SELECT DCount('*','Items') FROM Items",
            QueryError::Unsupported,
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            matches!(prepare_with_parameter_discovery(query.query(),&data,PreparationOptions::default(),&mut budget(),&||false),Err(actual) if actual==expected),
            "{sql}"
        );
    }
    assert_eq!(data.scans.get(), 0);
}

#[test]
fn schema_drift_cancellation_and_budget_exhaustion_preserve_checked_boundaries() {
    let mut data = Data::new();
    let query = parse_query(
        "SELECT Id FROM Items WHERE Score>[minimum]",
        ParseLimits::default(),
    )
    .unwrap();
    let prepared = prepare_with_parameter_discovery(
        query.query(),
        &data,
        PreparationOptions::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    data.columns[1] = Column::new("Different".into(), ScalarType::Int32, true).unwrap();
    assert!(matches!(
        prepared.execute_collections(
            &data,
            &[("minimum", Value::Int32(6))],
            ExecutionOptions::default()
        ),
        Err(QueryError::SchemaChanged)
    ));
    assert_eq!(data.scans.get(), 0);
    assert!(matches!(
        prepare_with_parameter_discovery(
            query.query(),
            &data,
            PreparationOptions::default(),
            &mut budget(),
            &|| true
        ),
        Err(QueryError::Cancelled)
    ));
    assert!(matches!(
        prepare_with_parameter_discovery(
            query.query(),
            &data,
            PreparationOptions::default(),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(QueryError::Limit(_))
    ));
    assert_eq!(data.scans.get(), 0);
}

#[test]
fn positive_metadata_mode_preserves_existing_named_domain_and_crosstab_paths() {
    use super::{ParameterDiscoveryMode, parameter_discovery_mode};
    let data = Data::new();
    for sql in [
        "SELECT DCount('*','Items') FROM Items",
        "TRANSFORM Count(*) SELECT Id FROM Items GROUP BY Id PIVOT Score",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            parameter_discovery_mode(
                parsed.query(),
                &data,
                Default::default(),
                &mut budget(),
                &|| false
            )
            .unwrap(),
            ParameterDiscoveryMode::Strict
        );
    }
    let definition = parse_query("SELECT Id FROM Items", ParseLimits::default()).unwrap();
    struct Named<'a> {
        data: &'a Data,
        definition: &'a tablecraft_sql::query::Query,
    }
    impl Source for Named<'_> {
        fn query(&self, name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
            Ok((name == "Saved").then_some(self.definition))
        }
        fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
            self.data.columns(name)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("metadata mode must not scan");
        }
    }
    let named = Named {
        data: &data,
        definition: definition.query(),
    };
    let parsed = parse_query("SELECT Id FROM Saved", ParseLimits::default()).unwrap();
    assert_eq!(
        parameter_discovery_mode(
            parsed.query(),
            &named,
            Default::default(),
            &mut budget(),
            &|| false
        )
        .unwrap(),
        ParameterDiscoveryMode::Strict
    );
    assert_eq!(data.scans.get(), 0);
}
