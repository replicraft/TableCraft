//! Independent predicate membership never supplies mutable target row identities.
use super::{ReadSourceUpdateOptions, prepare_read_source_batch_update_with_options};
use crate::{
    ExternalSourceRequest, MutationOptions, QueryError, ResolvedExternalSource, Source,
    TextCollationId, UpdateError,
};
use std::{cell::Cell, rc::Rc};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, Snapshot, SnapshotError, UpdateProfile,
};
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
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

struct Editor {
    snapshot: Snapshot,
    finishes: Rc<Cell<usize>>,
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
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.finishes.set(self.finishes.get() + 1);
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

fn editor() -> (Editor, Rc<Cell<usize>>) {
    let finishes = Rc::new(Cell::new(0));
    let snapshot = Snapshot::new(
        "Items".into(),
        columns(),
        vec![
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Null],
        ],
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
            finishes: Rc::clone(&finishes),
        },
        finishes,
    )
}

struct Reads {
    target_columns: Vec<Column>,
    child_columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    scans: Cell<usize>,
    policy_calls: Cell<usize>,
    named_callbacks: Cell<usize>,
    external_callbacks: Cell<usize>,
    policies: [Vec<Option<TextCollationId>>; 2],
}

impl Source for Reads {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        match table {
            "Items" => Ok(&self.target_columns),
            "Changes" => Ok(&self.child_columns),
            _ => Err(QueryError::UnknownName),
        }
    }

    fn query(&self, _: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
        self.named_callbacks.set(self.named_callbacks.get() + 1);
        Err(QueryError::Source)
    }

    fn external_source(
        &self,
        _: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        self.external_callbacks
            .set(self.external_callbacks.get() + 1);
        Err(QueryError::Source)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        if table != "Items" {
            return Ok(&[]);
        }
        let call = self.policy_calls.get();
        self.policy_calls.set(call + 1);
        Ok(&self.policies[usize::from(call != 0)])
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        // Root target values/ROW handles must come from the editor snapshot;
        // querying the target through the independent read source is a failure.
        if table != "Changes" {
            return Err(QueryError::Source);
        }
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}

fn reads() -> Reads {
    Reads {
        target_columns: columns(),
        child_columns: vec![Column::new("Id".into(), ScalarType::Int32, true).unwrap()],
        // Deliberately unsorted, duplicated and nullable child membership.
        rows: vec![
            vec![Value::Int32(3)],
            vec![Value::Int32(1)],
            vec![Value::Int32(1)],
            vec![Value::Null],
        ],
        scans: Cell::new(0),
        policy_calls: Cell::new(0),
        named_callbacks: Cell::new(0),
        external_callbacks: Cell::new(0),
        policies: [Vec::new(), Vec::new()],
    }
}

#[test]
fn aliases_duplicate_exists_and_null_membership_keep_original_target_handles() {
    for (predicate, ordinals) in [
        (
            "EXISTS (SELECT c.Id FROM Changes AS c WHERE c.Id=i.Id)",
            vec![0, 2],
        ),
        ("i.Id IN (SELECT Id FROM Changes)", vec![0, 2]),
        ("i.Id NOT IN (SELECT Id FROM Changes)", vec![]),
        (
            "NOT EXISTS (SELECT Id FROM Changes WHERE Changes.Id=i.Id)",
            vec![1],
        ),
    ] {
        let sql = format!("UPDATE Items AS i SET Score=Nz(i.Score,0)+100 WHERE {predicate}");
        let query = parse_query(&sql, ParseLimits::default()).unwrap();
        let (editor, finishes) = editor();
        let id = editor.snapshot().id();
        let source = reads();
        let prepared = prepare_read_source_batch_update_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceUpdateOptions {
                target_snapshot: id,
                mutation: MutationOptions::default(),
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(prepared.matched_rows(), ordinals.len(), "{predicate}");
        assert_eq!(finishes.get(), 0);
        let expected = ordinals
            .into_iter()
            .map(|ordinal| (ordinal, Value::Int32([110, 120, 100][ordinal])))
            .collect::<Vec<_>>();
        let actual = prepared
            .finish(&mut budget(), &|| false)
            .unwrap()
            .into_backend();
        assert_eq!(actual, vec![("Score".into(), expected)], "{predicate}");
        assert_eq!(finishes.get(), 1);
        assert!(source.scans.get() > 0);
    }
}

#[test]
fn every_assignment_uses_old_values_even_when_child_predicate_has_duplicate_matches() {
    let query = parse_query("UPDATE Items AS i SET Id=i.Score, Score=i.Id WHERE i.Id=1 AND EXISTS (SELECT Id FROM Changes WHERE Changes.Id=i.Id)", ParseLimits::default()).unwrap();
    let (editor, finishes) = editor();
    let id = editor.snapshot().id();
    let source = reads();
    let prepared = prepare_read_source_batch_update_with_options(
        editor,
        &source,
        query.query(),
        &mut budget(),
        ReadSourceUpdateOptions {
            target_snapshot: id,
            mutation: MutationOptions::default(),
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(prepared.matched_rows(), 1);
    assert_eq!(finishes.get(), 0);
    assert_eq!(
        prepared
            .finish(&mut budget(), &|| false)
            .unwrap()
            .into_backend(),
        vec![
            ("Id".into(), vec![(0, Value::Int32(10))]),
            ("Score".into(), vec![(0, Value::Int32(1))])
        ]
    );
    assert_eq!(finishes.get(), 1);
}

#[test]
fn late_and_eager_errors_and_ambiguous_names_never_publish_partial_backend_requests() {
    for (case, sql) in [
        "UPDATE Items AS i SET Score=10/(3-i.Id) WHERE EXISTS (SELECT Id FROM Changes WHERE Changes.Id=i.Id)",
        "UPDATE Items AS i SET Score=1 WHERE False AND EXISTS (SELECT Id FROM Changes WHERE 1/0=0)",
        "UPDATE Items AS i SET Score=1 WHERE EXISTS (SELECT c.Id FROM Changes AS c WHERE c.Id=missing.Id)",
        "UPDATE Items AS i SET Score=(SELECT Id FROM Changes) WHERE i.Id=1",
    ]
    .into_iter()
    .enumerate()
    {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let (editor, finishes) = editor();
        let id = editor.snapshot().id();
        let source = reads();
        let result = prepare_read_source_batch_update_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceUpdateOptions {
                target_snapshot: id,
                mutation: MutationOptions::default(),
            },
            &|| false,
        );
        if case < 2 {
            assert!(
                matches!(
                    result,
                    Err(UpdateError::Query(QueryError::Evaluation(
                        EvalError::DivisionByZero
                    )))
                ),
                "{sql}"
            );
        } else {
            assert!(result.is_err(), "{sql}");
        }
        assert_eq!(finishes.get(), 0, "{sql}");
    }
    let (other, _) = editor();
    let (editor, finishes) = editor();
    let query = parse_query(
        "UPDATE Items SET Score=1 WHERE EXISTS (SELECT Id FROM Changes)",
        ParseLimits::default(),
    )
    .unwrap();
    let source = reads();
    assert!(matches!(
        prepare_read_source_batch_update_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceUpdateOptions {
                target_snapshot: other.snapshot().id(),
                mutation: MutationOptions::default()
            },
            &|| false
        ),
        Err(UpdateError::Query(QueryError::Source))
    ));
    assert_eq!(source.scans.get(), 0);
    assert_eq!(finishes.get(), 0);
}

#[test]
fn changing_target_field_policy_refuses_before_predicate_scans_or_backend_finish() {
    for drift in [false, true] {
        let columns = vec![
            Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
            Column::new("Score".into(), ScalarType::Text, true).unwrap(),
        ];
        let finishes = Rc::new(Cell::new(0));
        let snapshot = Snapshot::new(
            "Items".into(),
            columns.to_vec(),
            vec![vec![Value::Int32(1), Value::Text("original".into())]],
            SchemaPolicy {
                max_name_bytes: 255,
                max_columns: 256,
                names: NameComparison::AsciiInsensitive,
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let id = snapshot.id();
        let editor = Editor {
            snapshot,
            finishes: Rc::clone(&finishes),
        };
        let mut source = reads();
        source.target_columns = columns;
        source.policies = [
            vec![None, Some(TextCollationId::from_host(41))],
            vec![
                None,
                Some(TextCollationId::from_host(if drift { 42 } else { 41 })),
            ],
        ];
        let query = parse_query(
            "UPDATE Items AS i SET Score=i.Score WHERE EXISTS (SELECT Id FROM Changes WHERE Changes.Id=i.Id)",
            ParseLimits::default(),
        ).unwrap();
        let result = prepare_read_source_batch_update_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceUpdateOptions {
                target_snapshot: id,
                mutation: MutationOptions::default(),
            },
            &|| false,
        );
        if drift {
            assert!(matches!(
                result,
                Err(UpdateError::Query(QueryError::SchemaChanged))
            ));
            assert_eq!(source.scans.get(), 0);
            assert_eq!(finishes.get(), 0);
        } else {
            assert_eq!(
                result
                    .unwrap()
                    .finish(&mut budget(), &|| false)
                    .unwrap()
                    .into_backend(),
                vec![("Score".into(), vec![(0, Value::Text("original".into()))])]
            );
            assert_eq!(finishes.get(), 1);
        }
    }
}

#[test]
fn unsupported_retrieval_and_callables_never_activate_provider_or_named_callbacks() {
    for sql in [
        "UPDATE Items AS i SET Score=1 WHERE EXISTS (SELECT Id FROM Changes IN 'file:untrusted.accdb')",
        "UPDATE Items AS i SET Score=1 WHERE i.Id=ANY (SELECT Id FROM Changes)",
        "UPDATE Items AS i SET Score=1 WHERE False AND EXISTS (SELECT Eval('1') FROM Changes)",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let (editor, finishes) = editor();
        let id = editor.snapshot().id();
        let source = reads();
        let result = prepare_read_source_batch_update_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceUpdateOptions {
                target_snapshot: id,
                mutation: MutationOptions::default(),
            },
            &|| false,
        );
        assert!(
            matches!(result, Err(UpdateError::Query(QueryError::Unsupported))),
            "{sql}"
        );
        assert_eq!(source.named_callbacks.get(), 0, "{sql}");
        assert_eq!(source.external_callbacks.get(), 0, "{sql}");
        assert_eq!(source.scans.get(), 0, "{sql}");
        assert_eq!(finishes.get(), 0, "{sql}");
    }
}
