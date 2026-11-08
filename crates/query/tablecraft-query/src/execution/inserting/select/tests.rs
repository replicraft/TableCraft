//! Original retrieval membership and destination intent precede one batch finish.
use super::*;
use std::{
    cell::{Cell, RefCell},
    io,
};
use tablecraft_core::values::Double;
use tablecraft_db::{InsertValue, OmittingInsertableTable};
use tablecraft_schema::{NameComparison, SchemaPolicy};
use tablecraft_sql::{ParsedQuery, parse_query};

struct Input<'a> {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    scans: Cell<usize>,
    named: Vec<(&'static str, Query)>,
    native: Option<&'a RefCell<ResourceBudget>>,
}

impl Input<'_> {
    fn ordinary() -> Self {
        Self {
            columns: vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
            ],
            rows: vec![
                vec![Value::Int32(1), Value::Text("first".into())],
                vec![Value::Int32(2), Value::Text("second".into())],
                vec![Value::Int32(3), Value::Null],
            ],
            scans: Cell::new(0),
            named: Vec::new(),
            native: None,
        }
    }
}

impl Source for Input<'_> {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .named
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if table.eq_ignore_ascii_case("Items") {
            Ok(&self.columns)
        } else {
            Err(QueryError::UnknownName)
        }
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.columns(table)?;
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            if let Some(native) = self.native {
                native
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?
                    .charge(Resource::Steps, 1)?;
            }
            visitor(row)?;
        }
        Ok(())
    }
}

struct Target<'a> {
    columns: Vec<Column>,
    calls: &'a Cell<usize>,
}

type Rows = Vec<Vec<Option<Value>>>;

impl<'a> Target<'a> {
    fn ordinary(calls: &'a Cell<usize>) -> Self {
        Self {
            columns: Input::ordinary().columns,
            calls,
        }
    }
}

impl InsertableTable for Target<'_> {
    type Error = io::Error;
    type Plan = Rows;

    fn name(&self) -> &str {
        "Items"
    }

    fn columns(&self) -> &[Column] {
        &self.columns
    }

    fn schema_policy(&self) -> SchemaPolicy {
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        }
    }

    fn requires_field_list(&self) -> bool {
        false
    }

    fn plan_insert(
        self,
        _: Vec<Value>,
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Rows, io::Error> {
        panic!("append SELECT must use one batch finish");
    }
}

impl CompleteInsertableBatch for Target<'_> {
    fn plan_insert_rows(
        self,
        rows: Vec<Vec<Value>>,
        budget: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Rows, io::Error> {
        self.calls.set(self.calls.get() + 1);
        budget
            .charge(Resource::Steps, 1)
            .map_err(io::Error::other)?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_iter().map(Some).collect())
            .collect())
    }
}

impl OmittingInsertableTable for Target<'_> {
    fn plan_insert_with_omissions(
        self,
        _: Vec<InsertValue>,
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Rows, io::Error> {
        panic!("append SELECT must use one presence batch finish");
    }
}

impl OmittingInsertableBatch for Target<'_> {
    fn plan_insert_rows_with_omissions(
        self,
        rows: Vec<Vec<InsertValue>>,
        budget: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Rows, io::Error> {
        self.calls.set(self.calls.get() + 1);
        budget
            .charge(Resource::Steps, 1)
            .map_err(io::Error::other)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|value| match value {
                        InsertValue::Supplied(value) => Some(value),
                        InsertValue::Omitted => None,
                    })
                    .collect()
            })
            .collect())
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 100_000, 10_000_000)
}

fn query(sql: &str) -> ParsedQuery {
    parse_query(sql, Default::default()).unwrap()
}

#[test]
fn self_append_preserves_original_membership_order_and_reordered_target_positions() {
    let input = Input::ordinary();
    let calls = Cell::new(0);
    let sql = query(
        "PARAMETERS minimum LONG; INSERT INTO Items (Label,Id) SELECT Label,Id+10 FROM Items WHERE Id>=minimum ORDER BY Id DESC",
    );
    let parameters = [("minimum", Value::Int32(2))];
    let staged = prepare_insert_select(
        Target::ordinary(&calls),
        sql.query(),
        &input,
        &mut budget(),
        &parameters,
        Default::default(),
    )
    .unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(input.scans.get(), 1);
    let rows = staged.finish(&mut budget()).unwrap();
    assert_eq!(
        rows,
        vec![
            vec![Some(Value::Int32(13)), Some(Value::Null)],
            vec![Some(Value::Int32(12)), Some(Value::Text("second".into()))]
        ]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn omitted_fields_and_explicit_null_remain_different_for_each_original_row() {
    for (sql, label) in [
        ("INSERT INTO Items (Id) SELECT Id+10 FROM Items", None),
        (
            "INSERT INTO Items (Id,Label) SELECT Id+10,NULL FROM Items",
            Some(Value::Null),
        ),
    ] {
        let input = Input::ordinary();
        let calls = Cell::new(0);
        let parsed = query(sql);
        let staged = prepare_insert_select_with_omissions(
            Target::ordinary(&calls),
            parsed.query(),
            &input,
            &mut budget(),
            &[],
            Default::default(),
        )
        .unwrap();
        let rows = staged.finish(&mut budget()).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row[1] == label));
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn invalid_target_mapping_and_static_width_refuse_without_scans_or_finish() {
    for sql in [
        "INSERT INTO Items (Id,id) SELECT Id,Id FROM Items",
        "INSERT INTO Items (Missing,Label) SELECT Id,Label FROM Items",
        "INSERT INTO Items SELECT Id FROM Items",
        "INSERT INTO Items (Id) SELECT Id FROM Items",
    ] {
        let input = Input::ordinary();
        let calls = Cell::new(0);
        let parsed = query(sql);
        assert!(
            prepare_insert_select(
                Target::ordinary(&calls),
                parsed.query(),
                &input,
                &mut budget(),
                &[],
                Default::default()
            )
            .is_err()
        );
        assert_eq!(input.scans.get(), 0, "{sql}");
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn late_retrieval_and_assignment_errors_never_reach_batch_finish() {
    for sql in [
        "INSERT INTO Items SELECT 120/(3-Id),Label FROM Items",
        "INSERT INTO Items SELECT Id,1/(3-Id) FROM Items",
        "INSERT INTO Items SELECT IIf(Id=3,NULL,Id),Label FROM Items",
        "INSERT INTO Items SELECT TOP 1 120/(3-Id),Label FROM Items ORDER BY Id",
    ] {
        let input = Input::ordinary();
        let calls = Cell::new(0);
        let parsed = query(sql);
        assert!(
            prepare_insert_select(
                Target::ordinary(&calls),
                parsed.query(),
                &input,
                &mut budget(),
                &[],
                Default::default()
            )
            .is_err(),
            "{sql}"
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn empty_source_still_consumes_one_batch_validation_and_native_pool_after_scans() {
    let native = RefCell::new(budget());
    let mut input = Input::ordinary();
    input.native = Some(&native);
    let calls = Cell::new(0);
    let parsed = query("INSERT INTO Items SELECT Id,Label FROM Items WHERE Id>99");
    let staged = prepare_insert_select(
        Target::ordinary(&calls),
        parsed.query(),
        &input,
        &mut budget(),
        &[],
        Default::default(),
    )
    .unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(input.scans.get(), 1);
    assert!(staged.finish(&mut native.borrow_mut()).unwrap().is_empty());
    assert_eq!(calls.get(), 1);
}

#[test]
fn native_finish_failure_does_not_retry_or_reset_query_work() {
    let input = Input::ordinary();
    let calls = Cell::new(0);
    let parsed = query("INSERT INTO Items SELECT Id,Label FROM Items");
    let mut query_work = budget();
    let staged = prepare_insert_select(
        Target::ordinary(&calls),
        parsed.query(),
        &input,
        &mut query_work,
        &[],
        Default::default(),
    )
    .unwrap();
    let remaining = query_work.remaining(Resource::Steps);
    assert!(matches!(
        staged.finish(&mut ResourceBudget::new(0, 0, 0)),
        Err(InsertError::Backend(_))
    ));
    assert_eq!(query_work.remaining(Resource::Steps), remaining);
    assert_eq!(calls.get(), 1);
    assert_eq!(input.scans.get(), 1);
}

#[test]
fn strict_parameters_and_cancellation_apply_to_prepare_and_consuming_finish() {
    let parsed = query(
        "PARAMETERS minimum LONG; INSERT INTO Items SELECT Id,Label FROM Items WHERE Id>=minimum",
    );
    for parameters in [
        vec![],
        vec![("other", Value::Int32(1))],
        vec![("minimum", Value::Text("1".into()))],
        vec![("minimum", Value::Int32(1)), ("extra", Value::Null)],
    ] {
        let input = Input::ordinary();
        let calls = Cell::new(0);
        assert!(
            prepare_insert_select(
                Target::ordinary(&calls),
                parsed.query(),
                &input,
                &mut budget(),
                &parameters,
                Default::default()
            )
            .is_err()
        );
        assert_eq!(input.scans.get(), 0);
        assert_eq!(calls.get(), 0);
    }
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let input = Input::ordinary();
    let calls = Cell::new(0);
    let parameters = [("minimum", Value::Int32(1))];
    let staged = prepare_insert_select(
        Target::ordinary(&calls),
        parsed.query(),
        &input,
        &mut budget(),
        &parameters,
        ExecutionOptions {
            cancelled: Some(&cancel),
            ..Default::default()
        },
    )
    .unwrap();
    cancelled.set(true);
    assert!(matches!(
        staged.finish(&mut budget()),
        Err(InsertError::Query(QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn shared_root_allowance_cancellation_and_limits_refuse_without_finish() {
    let parsed = query("INSERT INTO Items SELECT Id,Label FROM Items");
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 10_000_000),
        ResourceBudget::new(32 * 1024 * 1024, 0, 10_000_000),
        ResourceBudget::new(32 * 1024 * 1024, 100_000, 0),
    ] {
        let input = Input::ordinary();
        let calls = Cell::new(0);
        assert!(
            prepare_insert_select(
                Target::ordinary(&calls),
                parsed.query(),
                &input,
                &mut allowance,
                &[],
                Default::default()
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
    let input = Input::ordinary();
    let calls = Cell::new(0);
    let polls = Cell::new(0);
    let cancel = || {
        polls.set(polls.get() + 1);
        polls.get() > 20
    };
    assert!(
        prepare_insert_select(
            Target::ordinary(&calls),
            parsed.query(),
            &input,
            &mut budget(),
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn named_read_definitions_keep_original_root_parameter_scope() {
    let mut input = Input::ordinary();
    let named = query("SELECT Id,Label FROM Items WHERE Id>1");
    input.named.push(("Filtered", named.query().clone()));
    let parsed = query(
        "PARAMETERS minimum LONG; INSERT INTO Items SELECT Id,Label FROM Filtered WHERE Id>=minimum",
    );
    let calls = Cell::new(0);
    let rows = prepare_insert_select(
        Target::ordinary(&calls),
        parsed.query(),
        &input,
        &mut budget(),
        &[("minimum", Value::Int32(3))],
        Default::default(),
    )
    .unwrap()
    .finish(&mut budget())
    .unwrap();
    assert_eq!(rows, vec![vec![Some(Value::Int32(3)), Some(Value::Null)]]);
}

#[test]
fn readonly_preparation_and_legacy_values_apis_do_not_gain_append_execution() {
    let parsed = query("INSERT INTO Items SELECT Id,Label FROM Items");
    let input = Input::ordinary();
    assert!(crate::prepare(parsed.query(), &input).is_err());
    let calls = Cell::new(0);
    assert!(
        super::super::plan_insert(
            Target::ordinary(&calls),
            parsed.query(),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(input.scans.get(), 0);
    assert_eq!(calls.get(), 0);
}

#[test]
fn final_expression_states_and_assignment_carriers_are_not_silent_nulls_or_coercions() {
    let input = Input::ordinary();
    for expression in ["CVErr(2000)", "'123'", "True", "1.5"] {
        let parsed = query(&format!("INSERT INTO Items SELECT {expression},NULL"));
        let calls = Cell::new(0);
        assert!(
            prepare_insert_select(
                Target::ordinary(&calls),
                parsed.query(),
                &input,
                &mut budget(),
                &[],
                Default::default()
            )
            .is_err(),
            "{expression}"
        );
        assert_eq!(calls.get(), 0);
    }
    let mut input = Input::ordinary();
    input.columns[0] = Column::new("Id".into(), ScalarType::Double, false).unwrap();
    input.rows[0][0] = Value::Double(Double::from_float(1.0));
    input.rows.truncate(1);
    let calls = Cell::new(0);
    let parsed = query("INSERT INTO Items SELECT Id,Label FROM Items");
    assert!(
        prepare_insert_select(
            Target::ordinary(&calls),
            parsed.query(),
            &input,
            &mut budget(),
            &[],
            Default::default()
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn dynamic_named_schema_discovery_shares_pool_and_validates_root_inputs_first() {
    let mut input = Input::ordinary();
    input.rows.truncate(1);
    let definition = query("TRANSFORM Count(*) SELECT 1 AS G FROM Items GROUP BY 1 PIVOT Id");
    input.named.push(("Pivoted", definition.query().clone()));
    let parsed = query(
        "PARAMETERS minimum LONG; INSERT INTO Items SELECT G,CLng([1]) FROM Pivoted WHERE G>=minimum",
    );
    let calls = Cell::new(0);
    assert!(matches!(
        prepare_insert_select(
            Target::ordinary(&calls),
            parsed.query(),
            &input,
            &mut budget(),
            &[],
            Default::default()
        ),
        Err(InsertError::Query(QueryError::Parameter))
    ));
    assert_eq!(input.scans.get(), 0);
    let mut target = Target::ordinary(&calls);
    target.columns[1] = Column::new("Label".into(), ScalarType::Int32, true).unwrap();
    let rows = prepare_insert_select(
        target,
        parsed.query(),
        &input,
        &mut budget(),
        &[("minimum", Value::Int32(1))],
        Default::default(),
    )
    .unwrap()
    .finish(&mut budget())
    .unwrap();
    assert_eq!(
        rows,
        vec![vec![Some(Value::Int32(1)), Some(Value::Int32(1))]]
    );
    assert_eq!(input.scans.get(), 1);
    assert_eq!(calls.get(), 1);
}

#[test]
fn raw_strings_reach_only_final_text_assignment_and_top_omitted_values_are_not_published() {
    let input = Input::ordinary();
    let calls = Cell::new(0);
    let parsed = query("INSERT INTO Items SELECT 1,ChrW(65)");
    let rows = prepare_insert_select(
        Target::ordinary(&calls),
        parsed.query(),
        &input,
        &mut budget(),
        &[],
        Default::default(),
    )
    .unwrap()
    .finish(&mut budget())
    .unwrap();
    assert_eq!(
        rows,
        vec![vec![Some(Value::Int32(1)), Some(Value::Text("A".into()))]]
    );
    let parsed = query("INSERT INTO Items SELECT 1,ChrW(55296)");
    assert!(matches!(
        prepare_insert_select(
            Target::ordinary(&calls),
            parsed.query(),
            &input,
            &mut budget(),
            &[],
            Default::default()
        ),
        Err(InsertError::Query(QueryError::Evaluation(
            EvalError::UnrepresentableText
        )))
    ));
    assert_eq!(calls.get(), 1);

    let parsed = query(
        "INSERT INTO Items SELECT TOP 1 Id,IIf(Id=1,'ok',ChrW(55296)) FROM Items ORDER BY Id",
    );
    let rows = prepare_insert_select(
        Target::ordinary(&calls),
        parsed.query(),
        &input,
        &mut budget(),
        &[],
        Default::default(),
    )
    .unwrap()
    .finish(&mut budget())
    .unwrap();
    assert_eq!(
        rows,
        vec![vec![Some(Value::Int32(1)), Some(Value::Text("ok".into()))]]
    );
}

#[test]
fn union_promotion_and_aggregates_complete_before_destination_assignment() {
    let input = Input::ordinary();
    let calls = Cell::new(0);
    let mut target = Target::ordinary(&calls);
    target.columns[0] = Column::new("Id".into(), ScalarType::Double, false).unwrap();
    let parsed = query("INSERT INTO Items SELECT CLng(1),NULL UNION ALL SELECT CDbl(2),NULL");
    let rows = prepare_insert_select(
        target,
        parsed.query(),
        &input,
        &mut budget(),
        &[],
        Default::default(),
    )
    .unwrap()
    .finish(&mut budget())
    .unwrap();
    assert_eq!(
        rows,
        vec![
            vec![
                Some(Value::Double(Double::from_float(1.0))),
                Some(Value::Null)
            ],
            vec![
                Some(Value::Double(Double::from_float(2.0))),
                Some(Value::Null)
            ]
        ]
    );

    let mut target = Target::ordinary(&calls);
    target.columns[1] = Column::new("Label".into(), ScalarType::Int32, true).unwrap();
    let parsed = query("INSERT INTO Items SELECT Min(Id),Count(*) FROM Items HAVING Count(*)>1");
    let rows = prepare_insert_select(
        target,
        parsed.query(),
        &input,
        &mut budget(),
        &[],
        Default::default(),
    )
    .unwrap()
    .finish(&mut budget())
    .unwrap();
    assert_eq!(
        rows,
        vec![vec![Some(Value::Int32(1)), Some(Value::Int32(3))]]
    );
    assert_eq!(calls.get(), 2);
}
