//! Independent selections prove correlation never substitutes read-side identities.
use super::{ReadSourceDeleteOptions, prepare_read_source_delete_with_options};
use crate::execution::deleting::tests::{budget, source as editor_source};
use crate::{DeleteError, MutationOptions, QueryError, Source, plan_delete};
use std::cell::{Cell, RefCell};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{DeletableTable, SnapshotError};
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

struct Reads<'a> {
    columns: Vec<Column>,
    changes_columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    scans: Cell<usize>,
    metadata: Cell<usize>,
    drift: Option<Vec<Column>>,
    native: Option<&'a RefCell<ResourceBudget>>,
}

impl Source for Reads<'_> {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if !matches!(table, "Items" | "Changes") {
            return Err(QueryError::UnknownName);
        }
        if table == "Items" {
            let count = self.metadata.get();
            self.metadata.set(count + 1);
            if count != 0
                && let Some(columns) = &self.drift
            {
                return Ok(columns);
            }
        }
        Ok(if table == "Changes" {
            &self.changes_columns
        } else {
            &self.columns
        })
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if table != "Changes" {
            return Err(QueryError::Source);
        }
        self.scans.set(self.scans.get() + 1);
        if let Some(native) = self.native {
            native.borrow_mut().charge(Resource::Steps, 1)?;
        }
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}

fn reads(columns: &[Column]) -> Reads<'static> {
    Reads {
        columns: columns.to_vec(),
        changes_columns: vec![
            Column::new("Id".into(), ScalarType::Int32, true).unwrap(),
            Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
        ],
        // Deliberately different order/value from the target snapshot.
        rows: vec![
            vec![Value::Int32(3), Value::Int32(9)],
            vec![Value::Int32(1), Value::Int32(5)],
        ],
        scans: Cell::new(0),
        metadata: Cell::new(0),
        drift: None,
        native: None,
    }
}

#[test]
fn correlated_exists_and_membership_keep_original_order_and_null_semantics() {
    for (sql, expected) in [
        (
            "DELETE i.* FROM Items AS i WHERE EXISTS (SELECT c.Id FROM Changes AS c WHERE c.Id=i.Id)",
            vec![0, 2],
        ),
        (
            "DELETE FROM Items WHERE Id IN (SELECT Id FROM Changes)",
            vec![0, 2],
        ),
        (
            "DELETE FROM Items WHERE Id NOT IN (SELECT Id FROM Changes)",
            vec![1],
        ),
        (
            "DELETE FROM Items WHERE NOT EXISTS (SELECT Id FROM Changes WHERE Changes.Id=Items.Id)",
            vec![1],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let (editor, calls) = editor_source(false);
        let source = reads(editor.snapshot().columns());
        let id = editor.snapshot().id();
        let prepared = prepare_read_source_delete_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceDeleteOptions {
                target_snapshot: id,
                mutation: MutationOptions::default(),
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(prepared.matched_rows(), expected.len());
        assert_eq!(calls.get(), 0);
        assert_eq!(
            prepared
                .finish(&mut budget(), &|| false)
                .unwrap()
                .into_backend(),
            expected
        );
        assert_eq!(calls.get(), 1);
    }
    let query = parse_query(
        "DELETE FROM Items WHERE Id NOT IN (SELECT Id FROM Changes)",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, calls) = editor_source(false);
    let mut source = reads(editor.snapshot().columns());
    source.rows.push(vec![Value::Null, Value::Null]);
    let id = editor.snapshot().id();
    let prepared = prepare_read_source_delete_with_options(
        editor,
        &source,
        query.query(),
        &mut budget(),
        ReadSourceDeleteOptions {
            target_snapshot: id,
            mutation: MutationOptions::default(),
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(prepared.matched_rows(), 0);
    assert!(
        prepared
            .finish(&mut budget(), &|| false)
            .unwrap()
            .into_backend()
            .is_empty()
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn legacy_entry_and_hidden_retrieval_remain_closed_before_finish() {
    let query = parse_query(
        "DELETE FROM Items WHERE EXISTS (SELECT Id FROM Changes)",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, calls) = editor_source(false);
    assert!(matches!(
        plan_delete(editor, query.query(), &mut budget(), None, &|| false),
        Err(DeleteError::Query(QueryError::UnknownName))
    ));
    assert_eq!(calls.get(), 0);
    for (sql, wrong_target) in [
        (
            "DELETE FROM Items WHERE EXISTS (SELECT Count(*) FROM Changes)",
            false,
        ),
        (
            "DELETE FROM Items WHERE EXISTS (SELECT Eval('1') FROM Changes)",
            false,
        ),
        ("DELETE FROM Items WHERE Id=(SELECT Id FROM Changes)", false),
        (
            "DELETE Changes.* FROM Changes WHERE EXISTS (SELECT Id FROM Items)",
            true,
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let (editor, calls) = editor_source(false);
        let source = reads(editor.snapshot().columns());
        let id = editor.snapshot().id();
        let result = prepare_read_source_delete_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceDeleteOptions {
                target_snapshot: id,
                mutation: MutationOptions::default(),
            },
            &|| false,
        );
        if wrong_target {
            assert!(
                matches!(result, Err(DeleteError::Query(QueryError::Source))),
                "{sql}"
            );
        } else {
            assert!(
                matches!(result, Err(DeleteError::Query(QueryError::Unsupported))),
                "{sql}"
            );
        }
        assert_eq!(calls.get(), 0);
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn late_subquery_failure_and_wrong_snapshot_or_schema_have_no_finish() {
    let query = parse_query(
        "DELETE FROM Items WHERE EXISTS (SELECT Id FROM Changes WHERE 10 \\ (Score-5)>0)",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, calls) = editor_source(false);
    let source = reads(editor.snapshot().columns());
    let id = editor.snapshot().id();
    assert!(matches!(
        prepare_read_source_delete_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceDeleteOptions {
                target_snapshot: id,
                mutation: MutationOptions::default()
            },
            &|| false
        ),
        Err(DeleteError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(source.scans.get(), 1);
    assert_eq!(calls.get(), 0);

    let query = parse_query(
        "DELETE FROM Items WHERE EXISTS (SELECT Id FROM Changes)",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, calls) = editor_source(false);
    let (other, _) = editor_source(false);
    let source = reads(editor.snapshot().columns());
    assert!(matches!(
        prepare_read_source_delete_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceDeleteOptions {
                target_snapshot: other.snapshot().id(),
                mutation: MutationOptions::default()
            },
            &|| false
        ),
        Err(DeleteError::Query(QueryError::Source))
    ));
    assert_eq!(source.metadata.get(), 0);
    assert_eq!(calls.get(), 0);

    let (editor, calls) = editor_source(false);
    let mut source = reads(editor.snapshot().columns());
    source.drift = Some(vec![
        Column::new("Other".into(), ScalarType::Int32, false).unwrap(),
    ]);
    let id = editor.snapshot().id();
    assert!(matches!(
        prepare_read_source_delete_with_options(
            editor,
            &source,
            query.query(),
            &mut budget(),
            ReadSourceDeleteOptions {
                target_snapshot: id,
                mutation: MutationOptions::default()
            },
            &|| false
        ),
        Err(DeleteError::Query(QueryError::SchemaChanged))
    ));
    assert_eq!(source.scans.get(), 0);
    assert_eq!(calls.get(), 0);
}

#[test]
fn staging_releases_shared_native_pool_before_one_typed_backend_finish() {
    let native = RefCell::new(budget());
    let query = parse_query(
        "DELETE FROM Items WHERE Id IN (SELECT Id FROM Changes)",
        ParseLimits::default(),
    )
    .unwrap();
    let (mut editor, calls) = editor_source(false);
    editor.backend_steps = 17;
    let source = Reads {
        native: Some(&native),
        ..reads(editor.snapshot().columns())
    };
    let id = editor.snapshot().id();
    let prepared = prepare_read_source_delete_with_options(
        editor,
        &source,
        query.query(),
        &mut budget(),
        ReadSourceDeleteOptions {
            target_snapshot: id,
            mutation: MutationOptions::default(),
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(calls.get(), 0);
    // A zero allowance proves backend qualification occurs only in finish.
    assert!(matches!(
        prepared.finish(&mut ResourceBudget::new(0, 0, 0), &|| false),
        Err(DeleteError::Backend(SnapshotError::Limit(_)))
    ));
    assert_eq!(calls.get(), 0);
    assert!(native.try_borrow_mut().is_ok());

    let (editor, calls) = editor_source(false);
    let source = Reads {
        native: Some(&native),
        ..reads(editor.snapshot().columns())
    };
    let id = editor.snapshot().id();
    let prepared = prepare_read_source_delete_with_options(
        editor,
        &source,
        query.query(),
        &mut budget(),
        ReadSourceDeleteOptions {
            target_snapshot: id,
            mutation: MutationOptions::default(),
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        prepared
            .finish(&mut native.borrow_mut(), &|| false)
            .unwrap()
            .into_backend(),
        vec![0, 2]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn typed_inputs_empty_selection_and_cancellation_keep_checked_finish_boundary() {
    let query = parse_query(
        "PARAMETERS minimum LONG; DELETE FROM Items WHERE EXISTS (SELECT Id FROM Changes WHERE Changes.Id=Items.Id AND Score>minimum)",
        ParseLimits::default(),
    ).unwrap();
    let (editor, calls) = editor_source(false);
    let source = reads(editor.snapshot().columns());
    let id = editor.snapshot().id();
    let parameters = [("minimum", Value::Int32(6))];
    let prepared = prepare_read_source_delete_with_options(
        editor,
        &source,
        query.query(),
        &mut budget(),
        ReadSourceDeleteOptions {
            target_snapshot: id,
            mutation: MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        prepared
            .finish(&mut budget(), &|| false)
            .unwrap()
            .into_backend(),
        vec![2]
    );
    assert_eq!(calls.get(), 1);

    let query = parse_query(
        "DELETE FROM Items WHERE EXISTS (SELECT Id FROM Changes)",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, calls) = editor_source(true);
    let source = reads(editor.snapshot().columns());
    let id = editor.snapshot().id();
    let prepared = prepare_read_source_delete_with_options(
        editor,
        &source,
        query.query(),
        &mut budget(),
        ReadSourceDeleteOptions {
            target_snapshot: id,
            mutation: MutationOptions::default(),
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(prepared.matched_rows(), 0);
    assert_eq!(source.scans.get(), 0);
    assert!(
        prepared
            .finish(&mut budget(), &|| false)
            .unwrap()
            .into_backend()
            .is_empty()
    );
    assert_eq!(calls.get(), 1);

    for cancelled in [false, true] {
        let (editor, calls) = editor_source(false);
        let source = reads(editor.snapshot().columns());
        let id = editor.snapshot().id();
        let result = prepare_read_source_delete_with_options(
            editor,
            &source,
            query.query(),
            &mut ResourceBudget::new(0, 0, 0),
            ReadSourceDeleteOptions {
                target_snapshot: id,
                mutation: MutationOptions::default(),
            },
            &|| cancelled,
        );
        if cancelled {
            assert!(matches!(
                result,
                Err(DeleteError::Query(QueryError::Cancelled))
            ));
        } else {
            assert!(matches!(
                result,
                Err(DeleteError::Query(QueryError::Limit(_)))
            ));
        }
        assert_eq!(source.scans.get(), 0);
        assert_eq!(calls.get(), 0);
    }
}
