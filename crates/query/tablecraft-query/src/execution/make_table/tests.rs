//! Independent query-level make-table expectations; no destination publisher exists here.
use super::*;
use crate::source::{MemorySource, MemoryTable};
use std::cell::Cell;
use tablecraft_core::{limits::Resource, values::Decimal};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_schema::{Column, ScalarType, SchemaError, TypeDescriptor};
use tablecraft_sql::parse_query;

struct Observed {
    memory: MemorySource,
    scans: Cell<usize>,
    destination_error: Option<QueryError>,
    destination_query: Option<Query>,
}

impl Source for Observed {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(if name == "Archive" {
            self.destination_query.as_ref()
        } else {
            None
        })
    }
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if table == "Archive"
            && let Some(error) = self.destination_error
        {
            return Err(error);
        }
        self.memory.columns(table)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.memory.scan(table, visitor)
    }
}

fn source(ids: &[i32]) -> Observed {
    let columns = vec![
        Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
        Column::with_type("Label".into(), TypeDescriptor::text(8).unwrap(), true).unwrap(),
        Column::with_type(
            "Amount".into(),
            TypeDescriptor::decimal(20, 4).unwrap(),
            false,
        )
        .unwrap(),
    ];
    let rows = ids
        .iter()
        .map(|id| {
            vec![
                Value::Int32(*id),
                Value::Null,
                Value::Decimal(Decimal::new(12345, 4).unwrap()),
            ]
        })
        .collect();
    Observed {
        memory: MemorySource::new(vec![
            MemoryTable::new("Items".into(), columns, rows).unwrap(),
        ])
        .unwrap(),
        scans: Cell::new(0),
        destination_error: None,
        destination_query: None,
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn run(sql: &str, source: &dyn Source) -> Result<MakeTablePlan, MakeTableError> {
    plan_make_table(
        parse_query(sql, Default::default()).unwrap().into_query(),
        source,
        &mut budget(),
        &[],
        Default::default(),
    )
}

#[test]
fn make_table_copies_exact_values_and_query_descriptors_without_keys_generation_or_defaults() {
    let source = source(&[7, 7]);
    let plan = run(
        "SELECT ID AS KeyValue, Label, Amount INTO Archive FROM Items",
        &source,
    )
    .unwrap();
    assert_eq!(plan.destination(), "Archive");
    assert_eq!(
        plan.columns()
            .iter()
            .map(MakeTableColumn::name)
            .collect::<Vec<_>>(),
        ["KeyValue", "Label", "Amount"]
    );
    assert_eq!(
        plan.columns()[0].data_type(),
        TypeDescriptor::new(ScalarType::Int32)
    );
    assert!(!plan.columns()[0].output_nullable());
    assert_eq!(
        plan.columns()[1].data_type(),
        TypeDescriptor::text(8).unwrap()
    );
    assert_eq!(
        plan.columns()[2].data_type(),
        TypeDescriptor::decimal(20, 4).unwrap()
    );
    // Duplicate IDs are copied, not made unique/generated. NULL is not replaced
    // with empty Text or a source/destination type default.
    assert_eq!(
        plan.rows(),
        &[
            vec![
                Value::Int32(7),
                Value::Null,
                Value::Decimal(Decimal::new(12345, 4).unwrap())
            ],
            vec![
                Value::Int32(7),
                Value::Null,
                Value::Decimal(Decimal::new(12345, 4).unwrap())
            ]
        ]
    );
    let Statement::Read(read) = &plan.retrieval().statement else {
        panic!("retained source context");
    };
    let ReadBody::Select(select) = &read.body else {
        panic!("original SELECT");
    };
    assert!(select.into.is_none() && select.into_external.is_none());
    assert_eq!(source.scans.get(), 1);
    assert_eq!(plan.into_rows().len(), 2);
}

#[test]
fn known_empty_and_all_null_outputs_keep_query_constraints_without_row_inference() {
    let source = source(&[1]);
    let plan = run(
        "SELECT Label, Amount INTO Archive FROM Items WHERE ID < 0",
        &source,
    )
    .unwrap();
    assert!(plan.rows().is_empty());
    assert_eq!(
        plan.columns()[0].data_type(),
        TypeDescriptor::text(8).unwrap()
    );
    assert_eq!(
        plan.columns()[1].data_type(),
        TypeDescriptor::decimal(20, 4).unwrap()
    );
    let plan = run("SELECT Label INTO Archive FROM Items", &source).unwrap();
    assert_eq!(plan.rows(), &[vec![Value::Null]]);
    assert_eq!(
        plan.columns()[0].data_type().scalar_type(),
        ScalarType::Text
    );
}

#[test]
fn existing_external_and_failed_destination_metadata_refuse_before_user_scans() {
    let mut source = source(&[1]);
    for sql in [
        "SELECT ID INTO Items FROM Items",
        "SELECT ID INTO Archive IN 'never-open.accdb' FROM Items",
    ] {
        assert!(matches!(
            run(sql, &source),
            Err(MakeTableError::DestinationExists | MakeTableError::ExternalDestination)
        ));
        assert_eq!(source.scans.get(), 0);
    }
    source.destination_error = Some(QueryError::SourceFailure(crate::SourceFailure::Busy));
    assert!(matches!(
        run("SELECT ID INTO Archive FROM Items", &source),
        Err(MakeTableError::Query(QueryError::SourceFailure(
            crate::SourceFailure::Busy
        )))
    ));
    assert_eq!(source.scans.get(), 0);
    source.destination_error = None;
    source.destination_query = Some(
        parse_query("SELECT 1/0 FROM NeverOpen", Default::default())
            .unwrap()
            .into_query(),
    );
    assert!(matches!(
        run("SELECT ID INTO Archive FROM Items", &source),
        Err(MakeTableError::DestinationExists)
    ));
    assert_eq!(
        source.scans.get(),
        0,
        "stored query collision must not execute its body"
    );
}

#[test]
fn null_only_dynamic_and_duplicate_output_declarations_are_not_guessed_from_values() {
    let source = source(&[7]);
    for sql in [
        "SELECT NULL AS Missing INTO Archive FROM Items",
        "SELECT IIf(ID=7,ID,'text') AS V INTO Archive FROM Items",
    ] {
        assert!(
            matches!(run(sql, &source), Err(MakeTableError::IndeterminateType)),
            "{sql}"
        );
    }
    assert!(matches!(
        run(
            "SELECT ID AS KeyValue, ID AS keyvalue INTO Archive FROM Items",
            &source
        ),
        Err(MakeTableError::Query(QueryError::Schema(
            SchemaError::Duplicate
        )))
    ));
}

#[test]
fn parameters_and_selected_declaration_categories_survive_moving_the_original_query() {
    let source = source(&[7]);
    let syntax = parse_query(
        "PARAMETERS p LONG; SELECT p AS V INTO Archive FROM Items",
        Default::default(),
    )
    .unwrap()
    .into_query();
    let plan = plan_make_table(
        syntax,
        &source,
        &mut budget(),
        &[("p", Value::Int32(23))],
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.rows(), &[vec![Value::Int32(23)]]);
    assert_eq!(plan.retrieval().parameters.len(), 1);
    let query = parse_query(
        "SELECT '2'+CVar(CInt(1)) AS V INTO Archive FROM Items",
        Default::default(),
    )
    .unwrap()
    .into_query();
    let plan = plan_make_table(
        query,
        &source,
        &mut budget(),
        &[],
        ExecutionOptions {
            declared_integral: true,
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        plan.columns()[0].data_type().scalar_type(),
        ScalarType::Text
    );
    assert_eq!(plan.rows(), &[vec![Value::Text("21".into())]]);
}

#[test]
fn top_and_false_filters_do_not_hide_required_eager_expression_failures() {
    let source = source(&[7, 2]);
    for sql in [
        "SELECT TOP 1 10/(ID-2) AS V INTO Archive FROM Items ORDER BY ID DESC",
        "SELECT ID INTO Archive FROM Items WHERE False AND 1/0=0",
    ] {
        assert!(
            matches!(
                run(sql, &source),
                Err(MakeTableError::Query(QueryError::Evaluation(
                    EvalError::DivisionByZero
                )))
            ),
            "{sql}"
        );
    }
}

#[test]
fn make_table_keeps_caller_budgets_and_cancellation_without_any_partial_plan() {
    let source = source(&[1]);
    for (mut budget, resource) in [
        (ResourceBudget::new(0, u64::MAX, u64::MAX), Resource::Bytes),
        (ResourceBudget::new(u64::MAX, 0, u64::MAX), Resource::Rows),
        (ResourceBudget::new(u64::MAX, u64::MAX, 0), Resource::Steps),
    ] {
        let query = parse_query("SELECT ID INTO Archive FROM Items", Default::default())
            .unwrap()
            .into_query();
        assert!(
            matches!(plan_make_table(query, &source, &mut budget, &[], Default::default()),
            Err(MakeTableError::Query(QueryError::Limit(limit))) if limit.resource == resource)
        );
    }
    let query = parse_query("SELECT ID INTO Archive FROM Items", Default::default())
        .unwrap()
        .into_query();
    assert!(matches!(
        plan_make_table(
            query,
            &source,
            &mut budget(),
            &[],
            ExecutionOptions {
                cancelled: Some(&|| true),
                ..Default::default()
            }
        ),
        Err(MakeTableError::Query(QueryError::Cancelled))
    ));
}
