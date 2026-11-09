//! New profile comparison safety does not widen frozen creation languages.
use super::*;
use crate::{CreateColumn, create_ace16_database};
use tablecraft_schema::ScalarType;

#[test]
fn unicode_requests_and_constructively_valid_unicode_source_names_refuse() {
    let before = raw::source(raw::SourceCase::Number);
    for (table, column) in [(TABLE, "Σ"), ("Tāble1", "AfterOff")] {
        assert!(matches!(
            image(&before).prepare_created_add_field_after_off_candidate(
                table,
                column,
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false,
            ),
            Err(FieldAdditionAfterOffError::Namespace)
        ));
    }

    // These genuine constructors establish a valid existing namespace. They are
    // setup only; complete AFTER byte oracles remain the independent raw fixtures.
    let id = CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let prefix = CreateColumn::new("K", ScalarType::Int32);
    let seed = create_ace16_database(TABLE, &[id, prefix], &[], &mut budget(), &|| false).unwrap();
    let added = image(seed.as_bytes())
        .prepare_created_add_field_candidate(
            TABLE,
            "Score",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let off = image(added.after_bytes())
        .prepare_disable_name_auto_correct_after_field_history(&mut budget(), &|| false)
        .unwrap();
    image(off.after_bytes())
        .check_field_history_name_auto_correct_off_source(&mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        image(off.after_bytes()).prepare_created_add_field_after_off_candidate(
            TABLE,
            "K",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false,
        ),
        Err(FieldAdditionAfterOffError::Namespace)
    ));
}
