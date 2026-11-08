//! A retained one-table source exception does not change with the new target.
use super::{SCOPE, budget, image};
use crate::{
    AddTableError, CreateColumn, CreateDatabaseError, FirstTableDefinition,
    add_table_source_capabilities, read_table,
};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_creation_autonumber::{self as fixtures, Case};

const PAGE: usize = 4096;

fn legacy_source(native_type: u8) -> Vec<u8> {
    let mut bytes = fixtures::source(Case::EmptyPrimary).bytes;
    // Independent empty Id/Score source. Score becomes a variable, zero-width
    // Long Text/OLE declaration; there are no live payload references or rows.
    let descriptor = 7 * PAGE + 100;
    bytes[descriptor] = native_type;
    bytes[descriptor + 7..descriptor + 9].copy_from_slice(&0_u16.to_le_bytes());
    bytes[descriptor + 15] = 2;
    bytes[descriptor + 21..descriptor + 25].fill(0);
    bytes[7 * PAGE + 43..7 * PAGE + 45].copy_from_slice(&1_u16.to_le_bytes());
    if native_type == 12 {
        bytes[descriptor + 11..descriptor + 15].copy_from_slice(&[9, 4, 0, 1]);
    }
    bytes
}

fn generated() -> FirstTableDefinition {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32),
    ];
    FirstTableDefinition::capture("Generated", &columns, &mut budget(), &|| false).unwrap()
}

fn manual(keyed: bool) -> FirstTableDefinition {
    let key = CreateColumn::new("ActualKey", ScalarType::Int32);
    let key = if keyed {
        key.with_primary_key("ActualPrimary").unwrap()
    } else {
        key
    };
    let columns = [CreateColumn::new("First", ScalarType::Int32), key];
    FirstTableDefinition::capture_manual("Manual", &columns, &mut budget(), &|| false).unwrap()
}

#[test]
fn retained_single_table_lval_source_qualifies_consistently_for_generated_and_manual_targets() {
    for native_type in [12, 11] {
        let before = legacy_source(native_type);
        let captured = image(&before);
        let rows = read_table(
            &mut captured.reader(),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(rows.rows().is_empty());
        let capabilities =
            add_table_source_capabilities(&before, SCOPE, &mut budget(), &|| false).unwrap();
        assert!(capabilities.can_add_keyless() && capabilities.can_add_keyed());
        let generated = image(&before)
            .plan_add_table_legacy(generated(), &mut budget(), &|| false)
            .unwrap();
        assert!(!generated.requires_mixed_intent());
        assert_eq!(
            &generated.after_bytes()[7 * PAGE..before.len()],
            &before[7 * PAGE..]
        );
        for keyed in [false, true] {
            let plan = image(&before)
                .plan_add_table(manual(keyed), &mut budget(), &|| false)
                .unwrap();
            assert!(plan.requires_mixed_intent());
            assert_eq!(
                plan.after_bytes().len(),
                (if keyed { 13 } else { 12 }) * PAGE
            );
            assert_eq!(
                &plan.after_bytes()[7 * PAGE..before.len()],
                &before[7 * PAGE..]
            );
            let captured = image(plan.after_bytes());
            for table in ["Items", "Manual"] {
                assert!(
                    read_table(&mut captured.reader(), SCOPE, table, &mut budget(), &|| {
                        false
                    })
                    .unwrap()
                    .rows()
                    .is_empty()
                );
            }
            // The retained one-table exception does not authorize another LVAL
            // source in the repeated/mixed ledger after this append.
            let later =
                add_table_source_capabilities(plan.after_bytes(), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            assert!(!later.can_add_keyless() && !later.can_add_keyed());
            assert!(matches!(
                image(&before).plan_add_table_legacy(manual(keyed), &mut budget(), &|| false),
                Err(AddTableError::Create(CreateDatabaseError::Declaration))
            ));
        }
    }
}
