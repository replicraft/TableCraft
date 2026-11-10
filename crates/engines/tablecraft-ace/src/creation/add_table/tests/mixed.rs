//! Independent mixed ledgers qualify before append, first insertion and proof.
use super::{SCOPE, budget, image};
use crate::{
    AddTableError, CreateColumn, CreateDatabaseError, FirstTableDefinition, InsertFieldValue,
    InspectError, OmittedRowInsert, RowInsert, add_table_source_available,
    add_table_source_capabilities, read_table,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Value},
};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_mixed_add_table::{self as fixtures, Family};

const FAMILIES: [Family; 5] = [
    Family::Keyless,
    Family::Int32,
    Family::Int64,
    Family::Currency,
    Family::Generated,
];
const PAGE: usize = 4096;

fn definition(table: &str, family: Family) -> FirstTableDefinition {
    let scalar = match family {
        Family::Int64 => ScalarType::Int64,
        Family::Currency => ScalarType::Currency,
        _ => ScalarType::Int32,
    };
    let id = CreateColumn::new("Id", scalar);
    let id = if family.keyed() {
        id.with_primary_key("PrimaryId").unwrap()
    } else {
        id
    };
    let id = if matches!(family, Family::Generated) {
        id.with_incrementing_autonumber().unwrap()
    } else {
        id
    };
    let columns = [id, CreateColumn::new("Score", ScalarType::Int32)];
    if matches!(family, Family::Generated) {
        FirstTableDefinition::capture(table, &columns, &mut budget(), &|| false).unwrap()
    } else {
        FirstTableDefinition::capture_manual(table, &columns, &mut budget(), &|| false).unwrap()
    }
}

#[test]
fn every_mixed_pair_matches_independent_after_and_preserves_original_rows_and_opaque_bytes() {
    for first in FAMILIES {
        let before = fixtures::source(&[first]);
        for added in FAMILIES {
            let expected = fixtures::source(&[first, added]);
            let plan = image(&before.bytes)
                .plan_add_table(definition("Mixed2", added), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.after_bytes(), expected.bytes, "{first:?} -> {added:?}");
            assert_eq!(
                plan.source_profile() == crate::AddTableSourceProfile::Mixed,
                !matches!((first, added), (Family::Generated, Family::Generated))
            );
            let after = image(plan.after_bytes());
            let original = read_table(&mut after.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(original.rows(), before.first_rows);
            let new = read_table(&mut after.reader(), SCOPE, "Mixed2", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert!(new.rows().is_empty());
            assert_eq!(
                &plan.after_bytes()[7 * PAGE..before.bytes.len()],
                &before.bytes[7 * PAGE..]
            );
        }
    }
    let before = fixtures::source(&[
        Family::Keyless,
        Family::Int64,
        Family::Generated,
        Family::Currency,
    ]);
    let expected = fixtures::source(&[
        Family::Keyless,
        Family::Int64,
        Family::Generated,
        Family::Currency,
        Family::Keyless,
    ]);
    let plan = image(&before.bytes)
        .plan_add_table(
            definition("Mixed5", Family::Keyless),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.after_bytes(), expected.bytes);
    assert!(plan.source_profile() == crate::AddTableSourceProfile::Mixed);
}

#[test]
fn mixed_append_keeps_actual_manual_key_position_and_existing_first_insert_routes() {
    let before = fixtures::source(&[Family::Keyless]);
    for family in FAMILIES {
        let (scalar, value) = match family {
            Family::Int64 => (ScalarType::Int64, Value::Int64(7)),
            Family::Currency => (
                ScalarType::Currency,
                Value::Currency(Currency::from_scaled(75000)),
            ),
            _ => (ScalarType::Int32, Value::Int32(7)),
        };
        let definition = if matches!(family, Family::Generated) {
            definition("Mixed2", family)
        } else {
            let key = CreateColumn::new("ActualKey", scalar);
            let key = if family.keyed() {
                key.with_primary_key("ActualPrimary").unwrap()
            } else {
                key
            };
            let columns = [CreateColumn::new("First", ScalarType::Int32), key];
            FirstTableDefinition::capture_manual("Mixed2", &columns, &mut budget(), &|| false)
                .unwrap()
        };
        let plan = image(&before.bytes)
            .plan_add_table(definition, &mut budget(), &|| false)
            .unwrap();
        let (inserted, expected) = if matches!(family, Family::Generated) {
            let request = OmittedRowInsert::new(vec![
                InsertFieldValue::Omitted,
                InsertFieldValue::Supplied(Value::Int32(17)),
            ]);
            (
                image(plan.after_bytes())
                    .plan_ordinary_omitted_row_inserts("Mixed2", &[request], &mut budget(), &|| {
                        false
                    })
                    .unwrap(),
                vec![Value::Int32(1), Value::Int32(17)],
            )
        } else {
            let expected = vec![Value::Int32(17), value];
            (
                image(plan.after_bytes())
                    .plan_ordinary_row_inserts(
                        "Mixed2",
                        &[RowInsert::new(expected.clone())],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap(),
                expected,
            )
        };
        let after = image(inserted.after_bytes());
        assert_eq!(
            read_table(&mut after.reader(), SCOPE, "Mixed2", &mut budget(), &|| {
                false
            })
            .unwrap()
            .rows(),
            &[expected]
        );
        assert_eq!(
            read_table(&mut after.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap()
            .rows(),
            before.first_rows
        );
    }
}

#[test]
fn sixty_two_page_source_keeps_keyless_capacity_distinct_from_keyed_and_old_profile() {
    let before = fixtures::capacity_source();
    assert_eq!(before.bytes.len(), 62 * PAGE);
    let capabilities =
        add_table_source_capabilities(&before.bytes, SCOPE, &mut budget(), &|| false).unwrap();
    assert!(capabilities.can_add_keyless());
    assert!(!capabilities.can_add_keyed());
    assert!(!add_table_source_available(&before.bytes, SCOPE, &mut budget(), &|| false).unwrap());
    let mut families = vec![Family::Generated];
    families.extend([Family::Keyless; 27]);
    let expected = fixtures::source(&families);
    let plan = image(&before.bytes)
        .plan_add_table(
            definition("Mixed28", Family::Keyless),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.after_bytes(), expected.bytes);
    let full = add_table_source_capabilities(
        plan.after_bytes(),
        SCOPE,
        &mut ResourceBudget::new(0, 0, 1),
        &|| false,
    )
    .unwrap();
    assert!(!full.can_add_keyless() && !full.can_add_keyed());
    assert!(matches!(
        image(&before.bytes).plan_add_table(
            definition("Mixed28", Family::Generated),
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::Capacity)
    ));
    assert!(matches!(
        image(&before.bytes).plan_add_table_legacy(
            definition("Mixed28", Family::Generated),
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::Source)
    ));
}

#[test]
fn mixed_source_ownership_roles_and_primary_membership_refuse_without_legacy_fallback() {
    let source = fixtures::source(&[Family::Int32, Family::Keyless]);
    let map_offset = u16::from_le_bytes(
        source.bytes[3 * PAGE + 20..3 * PAGE + 22]
            .try_into()
            .unwrap(),
    ) as usize;
    let mut bad_owned = source.bytes.clone();
    bad_owned[3 * PAGE + map_offset + 12] |= 0x80;
    let mut bad_candidate = source.bytes.clone();
    bad_candidate[7 * PAGE + 59] ^= 1;
    let mut bad_root = source.bytes.clone();
    bad_root[9 * PAGE + 484] ^= 2;
    for (case, bad) in [
        ("owned map claims page 63", bad_owned),
        ("candidate reference changes slot", bad_candidate),
        ("primary key differs from live row", bad_root),
    ] {
        assert!(
            !add_table_source_capabilities(&bad, SCOPE, &mut budget(), &|| false)
                .unwrap()
                .can_add_keyless()
        );
        let failure = image(&bad)
            .plan_add_table(definition("Next", Family::Keyless), &mut budget(), &|| {
                false
            })
            .err();
        let expected = match case {
            "primary key differs from live row" => {
                matches!(
                    failure,
                    Some(AddTableError::Inspect(InspectError::Malformed))
                )
            }
            _ => matches!(failure, Some(AddTableError::Source)),
        };
        assert!(expected, "{case}: unexpected refusal {failure:?}");
    }
    assert!(matches!(
        image(&source.bytes).plan_add_table_legacy(
            definition("Next", Family::Generated),
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::Source)
    ));
    assert!(matches!(
        image(&source.bytes).plan_add_table_legacy(
            definition("Next", Family::Keyless),
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::Create(CreateDatabaseError::Declaration))
    ));
}

#[test]
fn mixed_proof_refuses_old_byte_changes_and_new_definition_forgery() {
    let before = fixtures::source(&[Family::Currency, Family::Keyless]);
    let captured = image(&before.bytes);
    let definition = definition("Next", Family::Keyless);
    let (geometry, _) =
        super::source::derive_geometry(&captured, false, &mut budget(), &|| false).unwrap();
    let source =
        super::source::qualify_geometry(&captured, geometry, false, &mut budget(), &|| false)
            .unwrap();
    let plan = image(&before.bytes)
        .plan_add_table(definition, &mut budget(), &|| false)
        .unwrap();
    for offset in [
        7 * PAGE + 3500,
        geometry.added.definition * PAGE + 40,
        geometry.added.data * PAGE + 4,
    ] {
        let mut changed = plan.after_bytes().to_vec();
        changed[offset] ^= 1;
        assert!(matches!(
            super::audit::verify(
                &captured,
                &changed,
                plan.definition(),
                &source,
                &mut budget(),
                &|| false
            ),
            Err(AddTableError::Preservation)
        ));
    }
}

#[test]
fn mixed_plan_pays_exact_resource_pools_and_cancellation_stays_terminal() {
    let before = fixtures::source(&[Family::Keyless, Family::Int64]);
    let invoke = |work: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
        image(&before.bytes).plan_add_table(definition("Next", Family::Currency), work, cancelled)
    };
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let mut measured = budget();
    let initial = pools.map(|pool| measured.remaining(pool));
    let polls = Cell::new(0);
    invoke(&mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let used = pools.map(|pool| {
        initial[match pool {
            Resource::Bytes => 0,
            Resource::Rows => 1,
            Resource::Steps => 2,
        }] - measured.remaining(pool)
    });
    invoke(&mut ResourceBudget::new(used[0], used[1], used[2]), &|| {
        false
    })
    .unwrap();
    for index in 0..3 {
        let mut short = used;
        short[index] -= 1;
        assert!(matches!(
            invoke(
                &mut ResourceBudget::new(short[0], short[1], short[2]),
                &|| false
            ),
            Err(AddTableError::Inspect(InspectError::Limit(_)))
                | Err(AddTableError::Create(CreateDatabaseError::Inspect(
                    InspectError::Limit(_)
                )))
        ));
    }
    for stop in [1, polls.get() / 2, polls.get()] {
        let count = Cell::new(0);
        assert!(matches!(
            invoke(&mut budget(), &|| {
                count.set(count.get() + 1);
                count.get() == stop
            }),
            Err(AddTableError::Inspect(InspectError::Cancelled))
                | Err(AddTableError::Create(CreateDatabaseError::Inspect(
                    InspectError::Cancelled
                )))
        ));
    }
}
