use tablecraft_testkit::access::mutation_fixture as fixture;

use std::io::Cursor;
use tablecraft_access::{NativeEditError, NativeEditor, editing::sql};
use tablecraft_ace::{NativeEditPlan, NativeImage, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_model::CatalogScope;

// Independent fixture layout, deliberately separate from production offsets.
const SECOND_ROW_SCORE_OFFSET: usize = 12 * 4096 + 4080;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}
fn editor(bytes: &[u8]) -> NativeEditor {
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    NativeEditor::new(image, "Items", &mut budget(), &|| false).unwrap()
}

fn through_neutral_contract<E: EditableTable<Error = NativeEditError, Plan = NativeEditPlan>>(
    editor: E,
) -> NativeEditPlan {
    assert_eq!(editor.update_profile(), UpdateProfile::ExistingInt32);
    let handle = editor.snapshot().row_handle(1).unwrap();
    editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(handle, Value::Int32(99))],
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn shared_snapshot_and_edit_contract_drive_a_native_preservation_plan() {
    let bytes = fixture::with_rule("ValidationRule", ">=0 And <=100");
    let plan = through_neutral_contract(editor(&bytes));
    assert_eq!(plan.before_bytes(), bytes);
    assert_eq!(plan.changed_rows(), 1);
    let table = read_table(
        &mut Cursor::new(plan.after_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows()[1], [Value::Int32(2), Value::Int32(99)]);
}

#[test]
fn foreign_snapshot_handles_wrong_carriers_and_unavailable_profiles_are_explicit() {
    let bytes = fixture::database();
    let first = editor(&bytes);
    let handle = first.snapshot().row_handle(0).unwrap();
    let second = editor(&bytes);
    assert_eq!(
        second
            .plan_updates(
                "Score",
                &[CellUpdate::new(handle, Value::Int32(1))],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(NativeEditError::Snapshot(SnapshotError::StaleRow))
    );
    assert_eq!(
        first
            .plan_updates(
                "Score",
                &[CellUpdate::new(handle, Value::Int64(1))],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(NativeEditError::Unsupported)
    );
    let mut ace = bytes;
    ace[4..20].copy_from_slice(b"Standard ACE DB\0");
    ace[20] = 2;
    assert_eq!(editor(&ace).update_profile(), UpdateProfile::Unavailable);
}

#[test]
fn query_update_planning_drives_the_neutral_contract_and_native_rule_gate() {
    let bytes = fixture::with_rule("ValidationRule", ">=0 And <=100");
    let query = tablecraft_sql::parse_query(
        "UPDATE Items SET Score=Score+7 WHERE Id=2",
        tablecraft_sql::ParseLimits::default(),
    )
    .unwrap();
    // Independent whole-image expectation: the second row's fixed Score slot.
    let mut expected = bytes.clone();
    let score_offset = SECOND_ROW_SCORE_OFFSET;
    assert_eq!(
        &expected[score_offset..score_offset + 4],
        &20_i32.to_le_bytes()
    );
    expected[score_offset..score_offset + 4].copy_from_slice(&27_i32.to_le_bytes());
    let expected_rows = read_table(
        &mut Cursor::new(&expected),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(expected_rows.rows()[1], [Value::Int32(2), Value::Int32(27)]);

    let image = NativeImage::capture(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = sql::plan_update(image, query.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(plan.matched_rows(), 1);
    let native = plan.into_backend();
    assert_eq!(native.before_bytes(), bytes);
    assert_eq!(native.after_bytes(), expected);
    let table = read_table(
        &mut Cursor::new(native.after_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows()[1], [Value::Int32(2), Value::Int32(27)]);
}

#[test]
fn shared_sql_shape_check_refuses_nonlocal_or_open_ended_updates() {
    for sql in [
        "SELECT 1",
        "UPDATE Items SET Id=2, Score=27",
        "PARAMETERS p LONG; UPDATE Items SET Score=p",
        "UPDATE [remote].[Items] SET Score=27",
    ] {
        let query =
            tablecraft_sql::parse_query(sql, tablecraft_sql::ParseLimits::default()).unwrap();
        assert!(matches!(
            sql::local_update_table(query.query()),
            Err(tablecraft_query::QueryError::Unsupported)
        ));
    }
}
