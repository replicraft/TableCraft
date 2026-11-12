//! Independent target records and read-side duplicates exercise actual handle selection.
use super::prepare_joined_delete_with_options;
use crate::{
    DeleteError, JoinedMutationOptions, MutationOptions, QueryError, Source,
    source::{MemorySource, MemoryTable, TextCollationId},
};
use std::{cell::Cell, rc::Rc};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{DeletableTable, RowHandle, Snapshot, SnapshotError};
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 100_000, 2_000_000)
}

fn columns() -> Vec<Column> {
    vec![
        Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
        Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
    ]
}

fn rows(values: &[(i32, Option<i32>)]) -> Vec<Vec<Value>> {
    values
        .iter()
        .map(|(id, value)| vec![Value::Int32(*id), value.map_or(Value::Null, Value::Int32)])
        .collect()
}

struct Deleter {
    snapshot: Snapshot,
    finishes: Rc<Cell<usize>>,
}

impl DeletableTable for Deleter {
    type Error = SnapshotError;
    type Plan = Vec<usize>;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn plan_deletes(
        self,
        selected: &[RowHandle],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.finishes.set(self.finishes.get() + 1);
        selected
            .iter()
            .map(|handle| self.snapshot.ordinal(*handle))
            .collect()
    }
}

fn deleter(values: &[(i32, Option<i32>)]) -> (Deleter, Rc<Cell<usize>>) {
    let snapshot = Snapshot::new(
        "Items".into(),
        columns(),
        rows(values),
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let finishes = Rc::new(Cell::new(0));
    (
        Deleter {
            snapshot,
            finishes: finishes.clone(),
        },
        finishes,
    )
}

struct Reads {
    memory: MemorySource,
    scans: Cell<usize>,
    metadata: Cell<usize>,
    target_metadata: Cell<usize>,
    policies: Cell<usize>,
    drift_columns: Option<Vec<Column>>,
    drift_policies: bool,
}

impl Source for Reads {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.metadata.set(self.metadata.get() + 1);
        if table == "Items" {
            self.target_metadata.set(self.target_metadata.get() + 1);
            if self.target_metadata.get() > 1
                && let Some(columns) = &self.drift_columns
            {
                return Ok(columns);
            }
        }
        self.memory.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        if table == "Items" {
            self.policies.set(self.policies.get() + 1);
            if self.drift_policies && self.policies.get() > 1 {
                return Ok(&[None, None]);
            }
        }
        Ok(&[])
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

fn reads(target: &[(i32, Option<i32>)], changes: &[(i32, Option<i32>)]) -> Reads {
    Reads {
        memory: MemorySource::new(vec![
            MemoryTable::new("Items".into(), columns(), rows(target)).unwrap(),
            MemoryTable::new("Changes".into(), columns(), rows(changes)).unwrap(),
        ])
        .unwrap(),
        scans: Cell::new(0),
        metadata: Cell::new(0),
        target_metadata: Cell::new(0),
        policies: Cell::new(0),
        drift_columns: None,
        drift_policies: false,
    }
}

fn plan(sql: &str, target: &[(i32, Option<i32>)], reads: &Reads) -> Vec<usize> {
    let (editor, finishes) = deleter(target);
    let id = editor.snapshot.id();
    let query = parse_query(sql, ParseLimits::default())
        .unwrap()
        .into_query();
    let staged = prepare_joined_delete_with_options(
        editor,
        reads,
        &query,
        &mut budget(),
        JoinedMutationOptions {
            target_snapshot: id,
            target_qualifier: "t",
            mutation: MutationOptions::default(),
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(finishes.get(), 0);
    let count = staged.matched_rows();
    let planned = staged.finish(&mut budget(), &|| false).unwrap();
    assert_eq!(planned.matched_rows(), count);
    assert_eq!(finishes.get(), 1);
    planned.into_backend()
}

#[test]
fn duplicate_matches_delete_each_original_equal_valued_record_once_in_target_order() {
    let target = [(1, Some(10)), (1, Some(10)), (2, None)];
    let source = reads(&target, &[(2, Some(9)), (1, Some(7)), (1, Some(8))]);
    assert_eq!(
        plan(
            "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id WHERE c.Score>0",
            &target,
            &source
        ),
        [0, 1, 2]
    );
    assert_eq!(
        source.scans.get(),
        1,
        "the selected target uses only original snapshot rows"
    );
}

#[test]
fn self_join_occurrence_and_outer_absence_preserve_actual_target_identity() {
    let target = [(1, Some(10)), (2, Some(20)), (3, None)];
    let source = reads(&[(2, Some(99))], &[]);
    assert_eq!(
        plan(
            "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Items AS other ON t.Id<other.Id",
            &target,
            &source
        ),
        [0]
    );
    for sql in [
        "DELETE DISTINCTROW t.* FROM Items AS t LEFT JOIN Changes AS c ON t.Id=c.Id WHERE c.Id IS NULL",
        "DELETE DISTINCTROW t.* FROM Changes AS c RIGHT JOIN Items AS t ON t.Id=c.Id WHERE c.Id IS NULL",
    ] {
        let source = reads(&target, &[(1, Some(7)), (4, Some(9))]);
        assert_eq!(plan(sql, &target, &source), [1, 2]);
    }
    let source = reads(&target, &[(4, Some(9))]);
    assert!(plan("DELETE DISTINCTROW t.* FROM Items AS t RIGHT JOIN Changes AS c ON t.Id=c.Id WHERE t.Id IS NULL", &target, &source).is_empty());
}

#[test]
fn repeated_matches_and_absent_targets_do_not_hide_late_eager_predicate_errors() {
    let target = [(1, Some(10))];
    for (sql, changes) in [
        (
            "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id WHERE c.Score=7 OR 1/(c.Score-8)>0",
            vec![(1, Some(7)), (1, Some(8))],
        ),
        (
            "DELETE DISTINCTROW t.* FROM Items AS t RIGHT JOIN Changes AS c ON t.Id=c.Id WHERE 1/0=0",
            vec![(4, Some(9))],
        ),
        (
            "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id WHERE False AND 1/0=0",
            vec![(1, Some(7))],
        ),
    ] {
        let source = reads(&target, &changes);
        let (editor, finishes) = deleter(&target);
        let id = editor.snapshot.id();
        let query = parse_query(sql, ParseLimits::default())
            .unwrap()
            .into_query();
        assert!(
            matches!(
                prepare_joined_delete_with_options(
                    editor,
                    &source,
                    &query,
                    &mut budget(),
                    JoinedMutationOptions {
                        target_snapshot: id,
                        target_qualifier: "t",
                        mutation: Default::default()
                    },
                    &|| false
                ),
                Err(DeleteError::Query(QueryError::Evaluation(
                    EvalError::DivisionByZero
                )))
            ),
            "{sql}"
        );
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn missing_affirmative_policy_cross_products_external_and_multiple_targets_refuse_before_metadata()
{
    let target = [(1, Some(10))];
    for sql in [
        "DELETE t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
        "DELETE DISTINCTROW FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
        "DELETE DISTINCTROW t.*,c.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
        "DELETE DISTINCTROW t.* FROM Items AS t,Changes AS c WHERE t.Id=c.Id",
        "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN [never-open.accdb].Changes AS c ON t.Id=c.Id",
        "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id WHERE t.Id IN (SELECT Id FROM [never-open.accdb].Other)",
    ] {
        let source = reads(&target, &[(1, Some(7))]);
        let (editor, finishes) = deleter(&target);
        let id = editor.snapshot.id();
        let query = parse_query(sql, ParseLimits::default())
            .unwrap()
            .into_query();
        assert!(
            matches!(
                prepare_joined_delete_with_options(
                    editor,
                    &source,
                    &query,
                    &mut budget(),
                    JoinedMutationOptions {
                        target_snapshot: id,
                        target_qualifier: "t",
                        mutation: Default::default()
                    },
                    &|| false
                ),
                Err(DeleteError::Query(QueryError::Unsupported))
            ),
            "{sql}"
        );
        assert_eq!(
            (source.metadata.get(), source.scans.get(), finishes.get()),
            (0, 0, 0)
        );
    }
}

#[test]
fn schema_or_policy_drift_refuses_before_scanning_and_empty_targets_finish_once() {
    let target = [(1, Some(10))];
    let sql = "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id";
    for policy in [false, true] {
        let mut source = reads(&target, &[(1, Some(7))]);
        if policy {
            source.drift_policies = true;
        } else {
            source.drift_columns = Some(vec![
                Column::new("Id".into(), ScalarType::Int32, true).unwrap(),
                columns().remove(1),
            ]);
        }
        let (editor, finishes) = deleter(&target);
        let id = editor.snapshot.id();
        let query = parse_query(sql, ParseLimits::default())
            .unwrap()
            .into_query();
        assert!(matches!(
            prepare_joined_delete_with_options(
                editor,
                &source,
                &query,
                &mut budget(),
                JoinedMutationOptions {
                    target_snapshot: id,
                    target_qualifier: "t",
                    mutation: Default::default()
                },
                &|| false
            ),
            Err(DeleteError::Query(QueryError::SchemaChanged))
        ));
        assert_eq!((source.scans.get(), finishes.get()), (0, 0));
    }
    let source = reads(&[], &[(1, Some(7))]);
    assert!(plan(sql, &[], &source).is_empty());
}

#[test]
fn typed_parameters_snapshot_preconditions_and_caller_allowances_remain_explicit() {
    let target = [(1, Some(10)), (2, None)];
    let source = reads(&target, &[(1, Some(7)), (2, Some(8))]);
    let query = parse_query("PARAMETERS p LONG; DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id WHERE c.Score=p", ParseLimits::default()).unwrap().into_query();
    let (editor, finishes) = deleter(&target);
    let id = editor.snapshot.id();
    let parameters = [("p", Value::Int32(8))];
    let staged = prepare_joined_delete_with_options(
        editor,
        &source,
        &query,
        &mut budget(),
        JoinedMutationOptions {
            target_snapshot: id,
            target_qualifier: "t",
            mutation: MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        staged
            .finish(&mut budget(), &|| false)
            .unwrap()
            .into_backend(),
        [1]
    );
    assert_eq!(finishes.get(), 1);
    for (mut work, cancelled) in [
        (ResourceBudget::new(0, u64::MAX, u64::MAX), false),
        (ResourceBudget::new(u64::MAX, 0, u64::MAX), false),
        (ResourceBudget::new(u64::MAX, u64::MAX, 0), false),
        (budget(), true),
    ] {
        let (editor, finishes) = deleter(&target);
        let id = editor.snapshot.id();
        let result = prepare_joined_delete_with_options(
            editor,
            &source,
            &query,
            &mut work,
            JoinedMutationOptions {
                target_snapshot: id,
                target_qualifier: "t",
                mutation: MutationOptions {
                    parameters: Some(&parameters),
                    ..Default::default()
                },
            },
            &|| cancelled,
        );
        assert!(matches!(
            result,
            Err(DeleteError::Query(
                QueryError::Limit(_) | QueryError::Cancelled
            ))
        ));
        assert_eq!(finishes.get(), 0);
    }
    let (editor, finishes) = deleter(&target);
    let (other, _) = deleter(&target);
    assert!(matches!(
        prepare_joined_delete_with_options(
            editor,
            &source,
            &query,
            &mut budget(),
            JoinedMutationOptions {
                target_snapshot: other.snapshot.id(),
                target_qualifier: "t",
                mutation: Default::default()
            },
            &|| false
        ),
        Err(DeleteError::Query(QueryError::Source))
    ));
    assert_eq!(finishes.get(), 0);
}
