//! Independent appends retain live tables, dropped identities and physical history.
use super::{audit, build, source};
use crate::{
    AddTableError, AddTableSourceProfile, CreateColumn, FirstTableDefinition, NativeImage,
    NativeImageSource, add_table_source_capabilities, inspect_table_schema, read_table,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        CASES, Case, readdition::Target as ExistingTarget,
    },
    canonical_retained_add_table::{self as raw, TARGETS, Target},
    canonical_table_drop::current::{self as dropped, History},
};

const SCOPE: CatalogScope = CatalogScope::new(3006);

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_for_table_append(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
        .unwrap()
}

fn definition(name: &str, target: Target) -> FirstTableDefinition {
    let id = CreateColumn::new("Id", ScalarType::Int32);
    let id = if target.keyed() {
        id.with_primary_key("PrimaryId").unwrap()
    } else {
        id
    };
    let generated = matches!(target, Target::GeneratedNumber | Target::GeneratedText);
    let id = if generated {
        id.with_incrementing_autonumber().unwrap()
    } else {
        id
    };
    let companion = if matches!(target, Target::GeneratedText) {
        CreateColumn::short_text("Name", 255).unwrap()
    } else {
        CreateColumn::new("Score", ScalarType::Int32)
    };
    let columns = [id, companion];
    if generated {
        FirstTableDefinition::capture(name, &columns, &mut budget(), &|| false).unwrap()
    } else {
        FirstTableDefinition::capture_manual(name, &columns, &mut budget(), &|| false).unwrap()
    }
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
    );
}

#[test]
fn retained_append_matches_independent_images_for_live_and_dropped_current_histories() {
    for case in CASES {
        for history in [
            History::Fields,
            History::Inserted,
            History::Deleted,
            History::Empty,
        ] {
            let original = dropped::source(case, ExistingTarget::ShortText, true, history);
            for retired in [false, true] {
                let retired_bytes;
                let before = if retired {
                    retired_bytes = dropped::after(&original);
                    retired_bytes.as_slice()
                } else {
                    original.bytes.as_slice()
                };
                let capabilities =
                    add_table_source_capabilities(before, SCOPE, &mut budget(), &|| false).unwrap();
                assert!(capabilities.can_add_keyed());
                assert!(capabilities.can_add_keyless());
                let name = if retired { "Table1" } else { "Additional" };
                for target in TARGETS {
                    let expected = raw::append(before, name, target);
                    let plan = image(before)
                        .plan_add_table(definition(name, target), &mut budget(), &|| false)
                        .unwrap_or_else(|error| {
                            panic!("{case:?} {history:?} retired={retired} {target:?}: {error:?}")
                        });
                    assert_eq!(plan.source_profile(), AddTableSourceProfile::Retained);
                    equal_image(plan.before_bytes(), before);
                    equal_image(plan.after_bytes(), &expected);
                    let schema = inspect_table_schema(
                        &mut Cursor::new(plan.after_bytes()),
                        SCOPE,
                        name,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(schema.definition_page() as usize, before.len() / 4096);
                    assert_eq!(schema.declared_rows(), 0);
                    assert_eq!(schema.columns().len(), 2);
                    if !retired {
                        let old_rows = read_table(
                            &mut Cursor::new(before),
                            SCOPE,
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let retained = read_table(
                            &mut Cursor::new(plan.after_bytes()),
                            SCOPE,
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        assert_eq!(retained.rows(), old_rows.rows());
                    }
                }
            }
        }
    }
}

#[test]
fn retained_append_repeats_across_live_tables_and_native_bitmap_base_windows() {
    let original = dropped::source(
        Case::MixedBinary,
        ExistingTarget::ShortText,
        true,
        History::Inserted,
    );
    let mut before = dropped::after(&original);
    for index in 0..18 {
        before = raw::append(
            &before,
            &format!("Existing{index}"),
            Target::GeneratedNumber,
        );
    }
    assert_eq!(before.len(), 65 * 4096);
    let available =
        add_table_source_capabilities(&before, SCOPE, &mut budget(), &|| false).unwrap();
    assert!(available.can_add_keyed() && available.can_add_keyless());
    let expected = raw::append(&before, "Beyond", Target::GeneratedText);
    let plan = image(&before)
        .plan_add_table(
            definition("Beyond", Target::GeneratedText),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    equal_image(plan.after_bytes(), &expected);
    assert_eq!(plan.after_bytes().len(), 68 * 4096);
    let schema = inspect_table_schema(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Beyond",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.definition_page(), 65);
    let maps = 3 * 4096;
    let old_count = u16::from_le_bytes(before[maps + 12..maps + 14].try_into().unwrap()) as usize;
    let first_new = u16::from_le_bytes(
        plan.after_bytes()[maps + 14 + old_count * 2..maps + 16 + old_count * 2]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(
        &plan.after_bytes()[maps + first_new + 1..maps + first_new + 5],
        &64_u32.to_le_bytes()
    );

    let mut current = dropped::after(&original);
    for (name, target) in [
        ("Table1", Target::GeneratedText),
        ("Manual", Target::ManualNumber),
        ("Keyless", Target::KeylessNumber),
    ] {
        let expected = raw::append(&current, name, target);
        let plan = image(&current)
            .plan_add_table(definition(name, target), &mut budget(), &|| false)
            .unwrap();
        equal_image(plan.after_bytes(), &expected);
        current = expected;
    }
    for name in ["Table1", "Manual", "Keyless"] {
        assert!(
            read_table(
                &mut Cursor::new(&current),
                SCOPE,
                name,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows()
            .is_empty()
        );
    }
}

#[test]
fn retained_append_keeps_source_binding_and_protected_spans_under_refusal() {
    let original = dropped::source(
        Case::MixedBinary,
        ExistingTarget::ShortText,
        true,
        History::Inserted,
    );
    assert!(matches!(
        image(&original.bytes).plan_add_table(
            definition("TABLE1", Target::GeneratedNumber),
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::DuplicateName)
    ));
    let before = dropped::after(&original);
    let declaration = definition("Replacement", Target::GeneratedText);
    let native = image(&before);
    let captured = source::capture(&native, &declaration, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        build::build(
            &image(&before),
            &declaration,
            &captured,
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::Preservation)
    ));
    let expected = raw::append(&before, "Replacement", Target::GeneratedText);
    for offset in [
        3 * 4096 + 14,
        4 * 4096 + 28,
        7 * 4096 + 20,
        8 * 4096 + 4090,
        9 * 4096 + 482,
        10 * 4096 + 4090,
        before.len() + 75,
    ] {
        let mut forged = expected.to_vec();
        forged[offset] ^= 1;
        let owner = NativeImageSource::from_vec(forged, &mut budget(), &|| false).unwrap();
        assert!(
            audit::verify(
                &native,
                &owner,
                &declaration,
                &captured,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    assert!(
        image(&before)
            .plan_add_table(
                definition("Replacement", Target::GeneratedText),
                &mut budget(),
                &|| true
            )
            .is_err()
    );
    assert!(
        image(&before)
            .plan_add_table(
                definition("Replacement", Target::GeneratedText),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    assert!(
        image(&before)
            .plan_add_table_mixed(
                definition("Replacement", Target::GeneratedText),
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    assert!(
        image(&before)
            .plan_add_table_legacy(
                definition("Replacement", Target::GeneratedText),
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    equal_image(native.as_bytes(), &before);
}
