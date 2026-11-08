//! Neutral controls for one original-snapshot, one-finish batch capability.
use std::{cell::Cell, rc::Rc};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, Snapshot, SnapshotError, UpdateProfile,
};
use tablecraft_query::{plan_batch_update, plan_batch_update_with_budgets};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

type Recorded = Vec<(String, Vec<(usize, Value)>)>;

struct Editor {
    snapshot: Snapshot,
    calls: Rc<Cell<usize>>,
    backend_steps: u64,
}

impl EditableTable for Editor {
    type Error = SnapshotError;
    type Plan = Recorded;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn update_profile(&self) -> UpdateProfile {
        UpdateProfile::StoredScalar
    }

    fn plan_updates(
        self,
        _: &str,
        _: &[CellUpdate],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        panic!("batch planner used the single-column finish")
    }
}

impl EditableBatch for Editor {
    fn plan_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        budget.charge(Resource::Steps, self.backend_steps)?;
        columns
            .iter()
            .map(|column| {
                let rows = column
                    .updates
                    .iter()
                    .map(|update| {
                        Ok((self.snapshot.ordinal(update.row())?, update.value().clone()))
                    })
                    .collect::<Result<Vec<_>, SnapshotError>>()?;
                Ok((column.column.to_owned(), rows))
            })
            .collect()
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}

fn create_editor() -> (Editor, Rc<Cell<usize>>) {
    let columns = vec![
        Column::new("Id".to_owned(), ScalarType::Int32, false).unwrap(),
        Column::new("Group".to_owned(), ScalarType::Int32, true).unwrap(),
        Column::new("Score".to_owned(), ScalarType::Int32, true).unwrap(),
    ];
    let rows = (1..=4)
        .map(|id| {
            vec![
                Value::Int32(id),
                Value::Int32(id * 10),
                Value::Int32(id * 100),
            ]
        })
        .collect();
    let snapshot = Snapshot::new(
        "Items".to_owned(),
        columns,
        rows,
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    (
        Editor {
            snapshot,
            calls: Rc::clone(&calls),
            backend_steps: 0,
        },
        calls,
    )
}

#[test]
fn all_right_hand_sides_use_original_rows_and_finish_once() {
    let (editor, calls) = create_editor();
    let query = parse_query(
        "UPDATE Items AS i SET i.[Group]=i.Score, i.Score=i.[Group] WHERE i.Id IN (1,3)",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_batch_update(editor, query.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.matched_rows(), 2);
    assert_eq!(
        result.into_backend(),
        vec![
            (
                "Group".to_owned(),
                vec![(0, Value::Int32(100)), (2, Value::Int32(300))]
            ),
            (
                "Score".to_owned(),
                vec![(0, Value::Int32(10)), (2, Value::Int32(30))]
            ),
        ]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn nullable_fields_and_small_integral_literals_keep_stored_scalar_law() {
    let (editor, calls) = create_editor();
    let query = parse_query(
        "UPDATE Items SET [Group]=NULL, Score=11 WHERE Id=2",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_batch_update(editor, query.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.matched_rows(), 1);
    assert_eq!(
        result.into_backend(),
        vec![
            ("Group".to_owned(), vec![(1, Value::Null)]),
            ("Score".to_owned(), vec![(1, Value::Int32(11))]),
        ]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn duplicate_canonical_targets_refuse_before_where_or_rhs_evaluation() {
    for text in [
        "UPDATE Items AS i SET i.Score=1, score=2 WHERE 1/0=0",
        "UPDATE Items SET [Group]=1, [group]=2 WHERE FALSE",
    ] {
        let (editor, calls) = create_editor();
        let query = parse_query(text, ParseLimits::default()).unwrap();
        let result = plan_batch_update(editor, query.query(), &mut budget(), None, &|| false);
        assert!(matches!(
            result,
            Err(tablecraft_query::UpdateError::Query(
                tablecraft_query::QueryError::DuplicateAssignment
            ))
        ));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn late_second_assignment_and_eager_errors_never_finish_a_partial_batch() {
    for text in [
        "UPDATE Items SET [Group]=Id, Score=2147483644+Id",
        "UPDATE Items SET [Group]=Id, Score=1 WHERE FALSE AND 1/0=0",
        "UPDATE Items SET [Group]=Id, Score='wrong'",
        "UPDATE Items SET [Group]=Id, Id=NULL WHERE Id=4",
    ] {
        let (editor, calls) = create_editor();
        let query = parse_query(text, ParseLimits::default()).unwrap();
        assert!(plan_batch_update(editor, query.query(), &mut budget(), None, &|| false).is_err());
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn cancellation_and_each_exhausted_pool_never_reach_backend_finish() {
    let query = parse_query(
        "UPDATE Items SET [Group]=Score, Score=[Group]",
        ParseLimits::default(),
    )
    .unwrap();
    for mut limits in [
        ResourceBudget::new(0, 100_000, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 100_000, 0),
    ] {
        let (editor, calls) = create_editor();
        assert!(plan_batch_update(editor, query.query(), &mut limits, None, &|| false).is_err());
        assert_eq!(calls.get(), 0);
    }
    let (editor, calls) = create_editor();
    let polls = Cell::new(0);
    assert!(
        plan_batch_update(editor, query.query(), &mut budget(), None, &|| {
            polls.set(polls.get() + 1);
            polls.get() >= 12
        })
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn normal_single_column_planner_does_not_gain_batch_capability() {
    let (editor, calls) = create_editor();
    let query = parse_query(
        "UPDATE Items SET [Group]=Score, Score=[Group]",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        tablecraft_query::plan_update(editor, query.query(), &mut budget(), None, &|| false,),
        Err(tablecraft_query::UpdateError::Query(
            tablecraft_query::QueryError::Unsupported
        ))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn both_column_vectors_exact_limits_and_final_cancel_precede_finish() {
    let query = parse_query(
        "UPDATE Items SET [Group]=Score, Score=[Group]",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, calls) = create_editor();
    let mut measured = budget();
    let bytes = measured.remaining(Resource::Bytes);
    let steps = measured.remaining(Resource::Steps);
    let result = plan_batch_update(editor, query.query(), &mut measured, None, &|| false).unwrap();
    assert_eq!(result.matched_rows(), 4);
    assert_eq!(calls.get(), 1);
    let bytes = bytes - measured.remaining(Resource::Bytes);
    let steps = steps - measured.remaining(Resource::Steps);

    let (editor, calls) = create_editor();
    let mut exact = ResourceBudget::new(bytes, 100_000, steps);
    let polls = Cell::new(0);
    let result = plan_batch_update(editor, query.query(), &mut exact, None, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(result.matched_rows(), 4);
    assert_eq!(calls.get(), 1);
    assert_eq!(exact.remaining(Resource::Bytes), 0);
    assert_eq!(exact.remaining(Resource::Steps), 0);

    for mut denied in [
        ResourceBudget::new(bytes - 1, 100_000, steps),
        ResourceBudget::new(bytes, 100_000, steps - 1),
    ] {
        let (editor, calls) = create_editor();
        assert!(plan_batch_update(editor, query.query(), &mut denied, None, &|| false).is_err());
        assert_eq!(calls.get(), 0);
    }

    let final_poll = polls.get();
    let (editor, calls) = create_editor();
    let mut exact = ResourceBudget::new(bytes, 100_000, steps);
    let polls = Cell::new(0);
    assert!(matches!(
        plan_batch_update(editor, query.query(), &mut exact, None, &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(tablecraft_query::UpdateError::Query(
            tablecraft_query::QueryError::Cancelled
        ))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn split_batch_query_refusals_leave_backend_pool_untouched() {
    let query = parse_query(
        "UPDATE Items SET [Group]=Score, Score=[Group]",
        ParseLimits::default(),
    )
    .unwrap();
    for mut query_budget in [
        ResourceBudget::new(0, 100_000, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 100_000, 0),
    ] {
        let (editor, calls) = create_editor();
        let mut native = ResourceBudget::new(7, 11, 13);
        let result = plan_batch_update_with_budgets(
            editor,
            query.query(),
            &mut query_budget,
            &mut native,
            None,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(tablecraft_query::UpdateError::Query(
                tablecraft_query::QueryError::Limit(_)
            ))
        ));
        assert_eq!(calls.get(), 0);
        assert_eq!(native, ResourceBudget::new(7, 11, 13));
    }
}

#[test]
fn split_batch_backend_failure_is_independent_of_successful_query_evaluation() {
    let query = parse_query(
        "UPDATE Items SET [Group]=Score, Score=[Group]",
        ParseLimits::default(),
    )
    .unwrap();
    let (mut editor, calls) = create_editor();
    editor.backend_steps = 1;
    let mut query_budget = budget();
    let mut native = ResourceBudget::new(7, 11, 0);
    let result = plan_batch_update_with_budgets(
        editor,
        query.query(),
        &mut query_budget,
        &mut native,
        None,
        &|| false,
    );
    let Err(tablecraft_query::UpdateError::Backend(SnapshotError::Limit(error))) = result else {
        panic!("the backend must retain its own exhausted step limit");
    };
    assert_eq!(error.resource, Resource::Steps);
    assert_eq!(error.remaining, 0);
    assert_eq!(calls.get(), 1);
    // Each original row is visited and then materialized as an expression row.
    assert_eq!(query_budget.remaining(Resource::Rows), 100_000 - 2 * 4);
    assert!(query_budget.remaining(Resource::Steps) < 1_000_000);
    assert_eq!(native, ResourceBudget::new(7, 11, 0));
}

#[test]
fn split_batch_preserves_original_snapshot_and_one_consuming_finish() {
    let query = parse_query(
        "UPDATE Items AS i SET i.[Group]=i.Score, i.Score=i.[Group] WHERE i.Id IN (1,3)",
        ParseLimits::default(),
    )
    .unwrap();
    let (mut editor, calls) = create_editor();
    editor.backend_steps = 3;
    let mut query_budget = budget();
    let mut native = ResourceBudget::new(0, 0, 3);
    let result = plan_batch_update_with_budgets(
        editor,
        query.query(),
        &mut query_budget,
        &mut native,
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(result.matched_rows(), 2);
    assert_eq!(
        result.into_backend(),
        vec![
            (
                "Group".to_owned(),
                vec![(0, Value::Int32(100)), (2, Value::Int32(300))]
            ),
            (
                "Score".to_owned(),
                vec![(0, Value::Int32(10)), (2, Value::Int32(30))]
            ),
        ]
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(native, ResourceBudget::new(0, 0, 0));
    // Each original row is visited and then materialized as an expression row.
    assert_eq!(query_budget.remaining(Resource::Rows), 100_000 - 2 * 4);
}
