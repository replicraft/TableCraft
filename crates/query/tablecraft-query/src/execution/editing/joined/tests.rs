//! Independent action selections retain snapshot identity through scalar joins.
use super::*;
use crate::{
    plan_batch_update,
    source::{MemorySource, MemoryTable},
};
use std::{cell::Cell, rc::Rc};
use tablecraft_db::{ColumnUpdates, EditableTable, Snapshot, SnapshotError};
use tablecraft_schema::{Column, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

type Changes = Vec<(String, Vec<(usize, Value)>)>;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}
fn columns() -> Vec<Column> {
    vec![
        Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
        Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
    ]
}
fn target_rows() -> Vec<Vec<Value>> {
    vec![
        vec![Value::Int32(1), Value::Int32(10)],
        vec![Value::Int32(2), Value::Int32(20)],
        vec![Value::Int32(3), Value::Null],
    ]
}
struct Editor {
    snapshot: Snapshot,
    calls: Rc<Cell<usize>>,
}
impl EditableTable for Editor {
    type Error = SnapshotError;
    type Plan = Changes;
    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    fn update_profile(&self) -> UpdateProfile {
        UpdateProfile::StoredScalar
    }
    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.plan_columns(&[ColumnUpdates { column, updates }], budget, cancelled)
    }
}
impl EditableBatch for Editor {
    fn plan_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        budget.charge(Resource::Steps, 1)?;
        self.calls.set(self.calls.get() + 1);
        columns
            .iter()
            .map(|column| {
                Ok((
                    column.column.into(),
                    column
                        .updates
                        .iter()
                        .map(|update| {
                            Ok((self.snapshot.ordinal(update.row())?, update.value().clone()))
                        })
                        .collect::<Result<Vec<_>, SnapshotError>>()?,
                ))
            })
            .collect()
    }
}
fn make_editor(empty: bool) -> (Editor, Rc<Cell<usize>>) {
    let calls = Rc::new(Cell::new(0));
    let snapshot = Snapshot::new(
        "Items".into(),
        columns(),
        if empty { Vec::new() } else { target_rows() },
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (
        Editor {
            snapshot,
            calls: Rc::clone(&calls),
        },
        calls,
    )
}
struct Reads {
    source: MemorySource,
    scans: Cell<usize>,
    changed: Vec<Column>,
    metadata: Cell<usize>,
    drift: bool,
}
impl Source for Reads {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if table.eq_ignore_ascii_case("Items") {
            let calls = self.metadata.get();
            self.metadata.set(calls + 1);
            if self.drift && calls > 0 {
                return Ok(&self.changed);
            }
        }
        self.source.columns(table)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.source.scan(table, visitor)
    }
}
fn reads(changes: Vec<Vec<Value>>) -> Reads {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            columns(),
            vec![
                vec![Value::Int32(2), Value::Int32(200)],
                vec![Value::Int32(1), Value::Int32(100)],
                vec![Value::Int32(3), Value::Int32(300)],
            ],
        )
        .unwrap(),
        MemoryTable::new(
            "Changes".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, true).unwrap(),
                Column::new("NewScore".into(), ScalarType::Int32, true).unwrap(),
            ],
            changes,
        )
        .unwrap(),
    ])
    .unwrap();
    Reads {
        source,
        scans: Cell::new(0),
        changed: vec![
            Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
            Column::new("Score".into(), ScalarType::Int64, true).unwrap(),
        ],
        metadata: Cell::new(0),
        drift: false,
    }
}
fn changes() -> Reads {
    reads(vec![
        vec![Value::Int32(2), Value::Int32(7)],
        vec![Value::Int32(1), Value::Int32(5)],
    ])
}
fn run(
    editor: Editor,
    source: &Reads,
    sql: &str,
    qualifier: &str,
) -> Result<UpdatePlan<Changes>, UpdateError<SnapshotError>> {
    let id = editor.snapshot.id();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    plan_joined_batch_update_with_options(
        editor,
        source,
        query.query(),
        &mut budget(),
        &mut budget(),
        JoinedMutationOptions {
            target_snapshot: id,
            target_qualifier: qualifier,
            mutation: MutationOptions::default(),
        },
        &|| false,
    )
}

#[test]
fn joined_inner_update_is_a_real_causal_extension_and_finishes_original_target_order() {
    let sql =
        "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=t.Score+c.NewScore";
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let (old, old_calls) = make_editor(false);
    assert!(matches!(
        plan_batch_update(old, parsed.query(), &mut budget(), None, &|| false),
        Err(UpdateError::Query(QueryError::Unsupported))
    ));
    assert_eq!(old_calls.get(), 0);
    let (editor, calls) = make_editor(false);
    let source = changes();
    let plan = run(editor, &source, sql, "t").unwrap();
    assert_eq!(plan.matched_rows(), 2);
    assert_eq!(
        plan.into_backend(),
        vec![(
            "Score".into(),
            vec![(0, Value::Int32(15)), (1, Value::Int32(27))]
        )]
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        source.scans.get(),
        1,
        "selected target is read from the frozen editor, not read Source"
    );
}

#[test]
fn self_join_selects_the_exact_target_occurrence_and_uses_original_rhs_values() {
    let (editor, calls) = make_editor(false);
    let plan = run(
        editor,
        &changes(),
        "UPDATE Items AS l INNER JOIN Items AS r ON l.Id=r.Id SET r.Score=l.Score-r.Score",
        "r",
    )
    .unwrap();
    assert_eq!(
        plan.into_backend(),
        vec![(
            "Score".into(),
            vec![
                (0, Value::Int32(90)),
                (1, Value::Int32(180)),
                (2, Value::Null)
            ]
        )]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn outer_join_nulls_and_missing_target_identity_remain_distinct() {
    for sql in [
        "UPDATE Items AS t LEFT JOIN Changes AS c ON t.Id=c.Id SET t.Score=c.NewScore",
        "UPDATE Changes AS c RIGHT JOIN Items AS t ON t.Id=c.Id SET t.Score=c.NewScore",
    ] {
        let (editor, calls) = make_editor(false);
        let plan = run(editor, &changes(), sql, "t").unwrap();
        assert_eq!(
            plan.into_backend(),
            vec![(
                "Score".into(),
                vec![(0, Value::Int32(5)), (1, Value::Int32(7)), (2, Value::Null)]
            )]
        );
        assert_eq!(calls.get(), 1);
    }
    let source = reads(vec![
        vec![Value::Int32(9), Value::Int32(99)],
        vec![Value::Null, Value::Int32(55)],
    ]);
    let (editor, calls) = make_editor(false);
    let plan = run(editor, &source, "UPDATE Items AS t RIGHT JOIN Changes AS c ON t.Id=c.Id SET t.Score=c.NewScore WHERE t.Id IS NULL", "t").unwrap();
    assert_eq!(plan.matched_rows(), 0);
    assert_eq!(plan.into_backend(), vec![("Score".into(), Vec::new())]);
    assert_eq!(calls.get(), 1);
}

#[test]
fn repeated_target_matches_refuse_and_late_eager_errors_never_finish_the_editor() {
    let source = reads(vec![
        vec![Value::Int32(1), Value::Int32(5)],
        vec![Value::Int32(1), Value::Int32(0)],
    ]);
    let (editor, calls) = make_editor(false);
    let result = run(
        editor,
        &source,
        "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=c.NewScore",
        "t",
    );
    assert!(matches!(
        result,
        Err(UpdateError::Query(QueryError::Unsupported))
    ));
    assert_eq!(calls.get(), 0);
    let (editor, calls) = make_editor(false);
    let result = run(
        editor,
        &source,
        "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=10 \\ c.NewScore",
        "t",
    );
    assert!(matches!(
        result,
        Err(UpdateError::Query(QueryError::Evaluation(
            tablecraft_expressions::EvalError::DivisionByZero
        )))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn stale_snapshot_and_target_schema_drift_refuse_before_any_scan() {
    let (foreign, _) = make_editor(false);
    let id = foreign.snapshot.id();
    let (target, calls) = make_editor(false);
    let source = changes();
    let parsed = parse_query(
        "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=7",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_joined_batch_update_with_options(
        target,
        &source,
        parsed.query(),
        &mut budget(),
        &mut budget(),
        JoinedMutationOptions {
            target_snapshot: id,
            target_qualifier: "t",
            mutation: MutationOptions::default(),
        },
        &|| false,
    );
    assert!(matches!(
        result,
        Err(UpdateError::Query(QueryError::Source))
    ));
    assert_eq!(source.scans.get(), 0);
    assert_eq!(calls.get(), 0);
    let (target, calls) = make_editor(false);
    let mut source = changes();
    source.drift = true;
    assert!(matches!(
        run(
            target,
            &source,
            "UPDATE Changes AS c INNER JOIN Items AS t ON t.Id=c.Id SET t.Score=7",
            "t"
        ),
        Err(UpdateError::Query(QueryError::SchemaChanged))
    ));
    assert_eq!(source.scans.get(), 0);
    assert_eq!(calls.get(), 0);
}

#[test]
fn parameters_no_matches_and_explicit_work_controls_keep_one_consuming_finish() {
    let (editor, calls) = make_editor(false);
    let id = editor.snapshot.id();
    let parsed = parse_query("PARAMETERS delta LONG; UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=t.Score+delta WHERE c.NewScore>0", ParseLimits::default()).unwrap();
    let values = [("delta", Value::Int32(2))];
    let plan = plan_joined_batch_update_with_options(
        editor,
        &changes(),
        parsed.query(),
        &mut budget(),
        &mut budget(),
        JoinedMutationOptions {
            target_snapshot: id,
            target_qualifier: "t",
            mutation: MutationOptions {
                parameters: Some(&values),
                ..Default::default()
            },
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        plan.into_backend(),
        vec![(
            "Score".into(),
            vec![(0, Value::Int32(12)), (1, Value::Int32(22))]
        )]
    );
    assert_eq!(calls.get(), 1);
    for cancelled in [false, true] {
        let (editor, calls) = make_editor(false);
        let id = editor.snapshot.id();
        let result = plan_joined_batch_update_with_options(
            editor,
            &changes(),
            parsed.query(),
            &mut ResourceBudget::new(0, 0, 0),
            &mut budget(),
            JoinedMutationOptions {
                target_snapshot: id,
                target_qualifier: "t",
                mutation: MutationOptions::default(),
            },
            &|| cancelled,
        );
        assert!(matches!(
            result,
            Err(UpdateError::Query(
                QueryError::Cancelled | QueryError::Limit(_)
            ))
        ));
        assert_eq!(calls.get(), 0);
    }
    let (editor, calls) = make_editor(true);
    let plan = run(
        editor,
        &changes(),
        "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=7",
        "t",
    )
    .unwrap();
    assert_eq!(plan.matched_rows(), 0);
    assert_eq!(calls.get(), 1);
}

#[test]
fn assignments_cannot_escape_the_selected_occurrence_or_repeat_one_target() {
    for (sql, expected) in [
        (
            "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET c.NewScore=77",
            QueryError::Unsupported,
        ),
        (
            "UPDATE Items AS t INNER JOIN Changes AS c ON t.Id=c.Id SET t.Score=7,t.Score=8",
            QueryError::DuplicateAssignment,
        ),
    ] {
        let (editor, calls) = make_editor(false);
        let source = changes();
        assert!(
            matches!(run(editor, &source, sql, "t"), Err(UpdateError::Query(error)) if error == expected)
        );
        assert_eq!(source.scans.get(), 0);
        assert_eq!(calls.get(), 0);
    }
}
