use super::{SCOPE, budget, int32_sources::model};
use std::io::Cursor;
use tablecraft_access::{
    NativeEditError,
    engineering::{AccessEngineeringEditor, ScalarUpdateMode, UpdateMode, sql},
};
use tablecraft_ace::{EditError, NativeImage, WriteBlocker};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable};
use tablecraft_sql::{ParseLimits, parse_query};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn qualify_preserved_int32_editor_snapshots_before_consumers() {
    for (code, value) in model::CASES {
        let case = model::source(code, value);
        for (bytes, first) in [(&case.before, 1000), (&case.expected, value)] {
            let editor = AccessEngineeringEditor::new(
                image(bytes),
                "Items",
                ScalarUpdateMode::Default,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let snapshot = editor.snapshot();
            let position = snapshot
                .columns()
                .iter()
                .position(|column| column.name() == "Large_Number")
                .unwrap();
            assert_eq!(snapshot.rows()[0][position], Value::Int32(first));
            assert_eq!(&bytes[case.patch..case.patch + 4], &first.to_le_bytes());
        }
    }
}

#[test]
fn preserved_int32_typed_and_sql_copies_follow_native_header_admission() {
    let mut positive = 0;
    let mut refused = 0;
    for (code, value) in model::CASES {
        let case = model::source(code, value);
        let editor = AccessEngineeringEditor::new(
            image(&case.before),
            "Items",
            ScalarUpdateMode::Default,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let handle = editor.snapshot().row_handle(0).unwrap();
        let typed = editor.plan_updates(
            "Large_Number",
            &[CellUpdate::new(handle, Value::Int32(value))],
            &mut budget(),
            &|| false,
        );
        if code == 5 {
            assert!(matches!(
                typed,
                Err(NativeEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
            refused += 1;
        } else {
            assert!(typed.unwrap().candidate_bytes() == case.expected);
            positive += 1;
        }
        for mode in [
            ScalarUpdateMode::Default,
            ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ] {
            let expression = if matches!(value, i32::MIN | i32::MAX) {
                format!("CLng({value})")
            } else {
                value.to_string()
            };
            let statement = format!("UPDATE Items SET Large_Number={expression} WHERE ID=1");
            let query = parse_query(&statement, ParseLimits::default()).unwrap();
            let plan = sql::preview_batch_update(
                image(&case.before),
                query.query(),
                mode,
                &mut budget(),
                None,
                &|| false,
            );
            if code == 5 {
                assert!(plan.is_err());
                refused += 1;
            } else {
                assert!(plan.unwrap().into_backend().candidate_bytes() == case.expected);
                positive += 1;
            }
        }
    }
    assert_eq!(positive, 12);
    assert_eq!(refused, 12);
}

#[test]
fn preserved_int32_refuses_dependent_indexed_and_joint_targets() {
    let case = model::source(6, 101);
    for statement in [
        "UPDATE Items SET [Number]=12 WHERE ID=1",
        "UPDATE Items SET [Number]=12 WHERE ID=-1",
        "UPDATE Items SET ID=12 WHERE ID=1",
        "UPDATE Items SET Calculated=12 WHERE ID=1",
        "UPDATE Items SET Large_Number=101,Yes_Field=True WHERE ID=1",
        "UPDATE Items SET Large_Number=101,[Number]=12 WHERE ID=1",
        "UPDATE Items SET Large_Number=Calculated WHERE ID=1",
        "UPDATE Items SET Large_Number=101 WHERE Attachment_Field IS NULL",
    ] {
        let query = parse_query(statement, ParseLimits::default()).unwrap();
        assert!(
            sql::preview_batch_update(
                image(&case.before),
                query.query(),
                ScalarUpdateMode::Default,
                &mut budget(),
                None,
                &|| false,
            )
            .is_err(),
            "{statement}"
        );
    }
}

#[test]
fn preserved_int32_null_copy_keeps_original_inactive_bytes_through_shared_services() {
    let case = model::source(6, 101);
    let mut expected = case.before.clone();
    expected[13 * 4096 - 2] &= !16;
    let editor = AccessEngineeringEditor::new(
        image(&case.before),
        "Items",
        ScalarUpdateMode::Default,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let handle = editor.snapshot().row_handle(0).unwrap();
    let typed = editor
        .plan_updates(
            "Large_Number",
            &[CellUpdate::new(handle, Value::Null)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(typed.candidate_bytes(), expected);

    let query = parse_query(
        "UPDATE Items SET Large_Number=NULL WHERE ID=1",
        ParseLimits::default(),
    )
    .unwrap();
    let sql = sql::preview_batch_update(
        image(&case.before),
        query.query(),
        ScalarUpdateMode::Default,
        &mut budget(),
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(sql.into_backend().candidate_bytes(), expected);
}
