//! Mutation function tests over a neutral original snapshot.
use std::{cell::Cell, rc::Rc};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, DeletableTable, EditableBatch, EditableTable, RowHandle, Snapshot,
    SnapshotError, UpdateProfile,
};
use tablecraft_query::{QueryError, UpdateError, plan_batch_update, plan_delete, plan_update};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

struct Editor {
    snapshot: Snapshot,
    finishes: Rc<Cell<usize>>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}

fn source(empty: bool) -> (Editor, Rc<Cell<usize>>) {
    let columns = vec![
        Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
        Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
        Column::new("Label".into(), ScalarType::Text, true).unwrap(),
    ];
    let rows = if empty {
        Vec::new()
    } else {
        vec![
            vec![
                Value::Int32(1),
                Value::Int32(-10),
                Value::Text("  alpha ".into()),
            ],
            vec![Value::Int32(2), Value::Null, Value::Text("Beta".into())],
            vec![Value::Int32(3), Value::Int32(3), Value::Null],
            vec![
                Value::Int32(4),
                Value::Int32(i32::MIN),
                Value::Text("last".into()),
            ],
        ]
    };
    let snapshot = Snapshot::new(
        "Items".into(),
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
    let finishes = Rc::new(Cell::new(0));
    (
        Editor {
            snapshot,
            finishes: Rc::clone(&finishes),
        },
        finishes,
    )
}

impl EditableTable for Editor {
    type Error = SnapshotError;
    type Plan = Vec<(usize, Value)>;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn update_profile(&self) -> UpdateProfile {
        UpdateProfile::StoredScalar
    }

    fn plan_updates(
        self,
        _: &str,
        updates: &[CellUpdate],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.finishes.set(self.finishes.get() + 1);
        updates
            .iter()
            .map(|update| Ok((self.snapshot.ordinal(update.row())?, update.value().clone())))
            .collect()
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
            .flat_map(|column| column.updates)
            .map(|update| Ok((self.snapshot.ordinal(update.row())?, update.value().clone())))
            .collect()
    }
}

impl DeletableTable for Editor {
    type Error = SnapshotError;
    type Plan = Vec<usize>;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn plan_deletes(
        self,
        handles: &[RowHandle],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.finishes.set(self.finishes.get() + 1);
        handles
            .iter()
            .map(|handle| self.snapshot.ordinal(*handle))
            .collect()
    }
}

#[test]
fn existing_numeric_functions_keep_carriers_and_finish_one_complete_selection() {
    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "UPDATE Items SET Score=CLng(Round(Abs(Nz(Score,0))/2.0,0)) WHERE Id<4",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.matched_rows(), 3);
    assert_eq!(
        result.into_backend(),
        vec![
            (0, Value::Int32(5)),
            (1, Value::Int32(0)),
            (2, Value::Int32(2))
        ]
    );
    assert_eq!(finishes.get(), 1);

    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "DELETE FROM Items WHERE Abs(Id-2)=0",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_delete(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.into_backend(), [1]);
    assert_eq!(finishes.get(), 1);
}

#[test]
fn safe_text_functions_preserve_nullable_results_and_have_no_ambient_policy() {
    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "UPDATE Items SET Label=Left(Trim(Label),3)",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(
        result.into_backend(),
        vec![
            (0, Value::Text("alp".into())),
            (1, Value::Text("Bet".into())),
            (2, Value::Null),
            (3, Value::Text("las".into())),
        ]
    );
    assert_eq!(finishes.get(), 1);
}

#[test]
fn unused_iif_nz_operands_and_late_overflow_are_still_eager() {
    for sql in [
        "UPDATE Items SET Score=IIf(Id<0,1/0,Score)",
        "UPDATE Items SET Score=Nz(Id,1/0)",
        "UPDATE Items SET Score=Abs(Score)",
        "UPDATE Items SET Score=1 WHERE FALSE AND Abs(Score)>0",
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(matches!(
            plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
            Err(UpdateError::Query(QueryError::Evaluation(_)))
        ));
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn domains_dynamic_runtime_policy_functions_and_udfs_refuse_before_any_finish() {
    for expression in [
        "DCount('*','Hidden')",
        "Eval('1')",
        "Rnd()",
        "Now()",
        "CurrentUser()",
        "UserFunction(Score)",
        "Other.Abs(Score)",
        "UCase(Label)",
        "DateAdd('d',1,#2020-01-01#)",
    ] {
        for empty in [false, true] {
            let (editor, finishes) = source(empty);
            let parsed = parse_query(
                &format!("UPDATE Items SET Score={expression} WHERE FALSE"),
                ParseLimits::default(),
            )
            .unwrap();
            assert!(matches!(
                plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
                Err(UpdateError::Query(QueryError::Unsupported))
            ));
            assert_eq!(finishes.get(), 0);
        }
    }
}

#[test]
fn duplicate_batch_columns_still_bind_before_any_function_operand() {
    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "UPDATE Items AS i SET i.Score=Abs(1/0), score=CLng(1/0)",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        plan_batch_update(editor, parsed.query(), &mut budget(), None, &|| false),
        Err(UpdateError::Query(QueryError::DuplicateAssignment))
    ));
    assert_eq!(finishes.get(), 0);
}

#[test]
fn wrong_function_arities_and_omitted_fallback_carriers_never_finish() {
    for expression in [
        "Abs()",
        "Round(1,2,3)",
        "Left(Label)",
        "Nz(Score)",
        "CLng(NULL)",
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression}"),
            ParseLimits::default(),
        )
        .unwrap();
        assert!(plan_update(editor, parsed.query(), &mut budget(), None, &|| false).is_err());
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn mutation_function_work_limits_and_actual_terminal_cancellation_precede_finish() {
    let parsed = parse_query(
        "UPDATE Items SET Label=Mid(RTrim(LTrim(Label)),1,3)",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, finishes) = source(false);
    let polls = Cell::new(0);
    plan_update(editor, parsed.query(), &mut budget(), None, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(finishes.get(), 1);
    let terminal = polls.get();

    let (editor, finishes) = source(false);
    let polls = Cell::new(0);
    assert!(matches!(
        plan_update(editor, parsed.query(), &mut budget(), None, &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        }),
        Err(UpdateError::Query(QueryError::Cancelled))
    ));
    assert_eq!(finishes.get(), 0);

    for mut allowance in [
        ResourceBudget::new(0, 100_000, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 100_000, 0),
    ] {
        let (editor, finishes) = source(false);
        assert!(plan_update(editor, parsed.query(), &mut allowance, None, &|| false).is_err());
        assert_eq!(finishes.get(), 0);
    }
}
