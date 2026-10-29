//! Neutral StoredScalar planning controls, independent of either native adapter.
use std::{cell::Cell, rc::Rc};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, Snapshot, SnapshotError, UpdateProfile};
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, UpdateError, plan_update};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

struct Editor {
    snapshot: Snapshot,
    profile: UpdateProfile,
    calls: Rc<Cell<usize>>,
}

impl EditableTable for Editor {
    type Error = SnapshotError;
    type Plan = Vec<usize>;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn update_profile(&self) -> UpdateProfile {
        self.profile
    }

    fn plan_updates(
        self,
        _: &str,
        updates: &[CellUpdate],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, SnapshotError> {
        self.calls.set(self.calls.get() + 1);
        updates
            .iter()
            .map(|update| self.snapshot.ordinal(update.row()))
            .collect()
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}

fn create_editor(profile: UpdateProfile) -> (Editor, Rc<Cell<usize>>) {
    let columns = vec![
        Column::new("Id".to_owned(), ScalarType::Int32, false).unwrap(),
        Column::new("Label".to_owned(), ScalarType::Text, false).unwrap(),
        Column::new("Other".to_owned(), ScalarType::Text, true).unwrap(),
        Column::new("Score".to_owned(), ScalarType::Int32, true).unwrap(),
    ];
    let rows = vec![
        vec![
            Value::Int32(1),
            Value::Text("A".to_owned()),
            Value::Text("X".to_owned()),
            Value::Int32(10),
        ],
        vec![
            Value::Int32(2),
            Value::Text("B".to_owned()),
            Value::Text("Y".to_owned()),
            Value::Int32(20),
        ],
        vec![
            Value::Int32(3),
            Value::Text("C".to_owned()),
            Value::Null,
            Value::Int32(30),
        ],
    ];
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
            profile,
            calls: calls.clone(),
        },
        calls,
    )
}

#[test]
fn stored_profile_selects_one_complete_batch_with_text_null_and_small_integer_literals() {
    for (text, expected) in [
        (
            "UPDATE Items AS i SET i.Label='new' WHERE i.Id IN (1,3)",
            vec![0, 2],
        ),
        ("UPDATE Items SET Score=11 WHERE Id>=2", vec![1, 2]),
        ("UPDATE Items SET Score=NULL WHERE Id=2", vec![1]),
    ] {
        let (editor, calls) = create_editor(UpdateProfile::StoredScalar);
        let query = parse_query(text, ParseLimits::default()).unwrap();
        let result = plan_update(editor, query.query(), &mut budget(), None, &|| false).unwrap();
        assert_eq!(result.matched_rows(), expected.len());
        assert_eq!(result.into_backend(), expected);
        assert_eq!(calls.get(), 1);
    }
    for text in [
        "UPDATE Items SET Label='new'",
        "UPDATE Items SET Score=NULL",
    ] {
        let (editor, calls) = create_editor(UpdateProfile::ExistingInt32);
        let query = parse_query(text, ParseLimits::default()).unwrap();
        assert!(matches!(
            plan_update(editor, query.query(), &mut budget(), None, &|| false),
            Err(UpdateError::Query(QueryError::Unsupported))
        ));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn late_nullability_and_eager_expression_errors_never_call_the_backend() {
    let (editor, calls) = create_editor(UpdateProfile::StoredScalar);
    let query = parse_query("UPDATE Items SET Label=Other", ParseLimits::default()).unwrap();
    assert!(matches!(
        plan_update(editor, query.query(), &mut budget(), None, &|| false),
        Err(UpdateError::Query(QueryError::Unsupported))
    ));
    assert_eq!(calls.get(), 0);
    for text in [
        "UPDATE Items SET Score=10 \\ (3-Id)",
        "UPDATE Items SET Score=11 WHERE Id<0 AND (1 \\ 0)=0",
    ] {
        let (editor, calls) = create_editor(UpdateProfile::StoredScalar);
        let query = parse_query(text, ParseLimits::default()).unwrap();
        assert!(matches!(
            plan_update(editor, query.query(), &mut budget(), None, &|| false),
            Err(UpdateError::Query(QueryError::Evaluation(
                EvalError::DivisionByZero
            )))
        ));
        assert_eq!(calls.get(), 0);
    }
    for text in [
        "UPDATE Items SET Label=11",
        "UPDATE Items SET Label=UCase(Label)",
        "UPDATE Items SET Score=(SELECT 1)",
    ] {
        let (editor, calls) = create_editor(UpdateProfile::StoredScalar);
        let query = parse_query(text, ParseLimits::default()).unwrap();
        assert!(plan_update(editor, query.query(), &mut budget(), None, &|| false).is_err());
        assert_eq!(calls.get(), 0);
    }
}
