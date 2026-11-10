//! Raw record images qualify replacement without changing retired or sibling roles.
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, SlotScalarUpdate,
    inspect_table_schema, read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        CASES, Case,
        readdition::{self, Target as Field},
    },
    canonical_retained_add_table::{
        self as raw, Target, desktop,
        rows::{self, Input, Profile},
        updates,
    },
    canonical_table_drop::current::{self as dropped, History},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(3717);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
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

fn source(case: Case, retired: bool, profile: Profile) -> (Vec<u8>, u32) {
    let original = dropped::source(case, Field::ShortText, true, History::Inserted);
    let before = if retired {
        dropped::after(&original)
    } else {
        original.bytes
    };
    let definition = u32::try_from(before.len() / 4096).unwrap();
    let target = match profile {
        Profile::NativeNumber | Profile::DesktopNumber => Target::GeneratedNumber,
        Profile::NativeText | Profile::DesktopText => Target::GeneratedText,
    };
    let mut bytes = match profile {
        Profile::DesktopNumber | Profile::DesktopText => desktop::append(&before, "Added", target),
        _ => raw::append(&before, "Added", target),
    };
    for _ in 0..3 {
        let input = match profile {
            Profile::NativeNumber | Profile::DesktopNumber => Input::Number(123),
            _ => Input::Text("initial"),
        };
        bytes = rows::append(&bytes, definition, profile, input);
    }
    (bytes, definition)
}

fn number_value(value: Option<i32>) -> Value {
    value.map_or(Value::Null, Value::Int32)
}

fn text_value(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |text| Value::Text(text.into()))
}

#[test]
fn retained_number_cells_match_raw_images_across_live_and_retired_surroundings() {
    for case in CASES {
        for retired in [false, true] {
            for profile in [Profile::NativeNumber, Profile::DesktopNumber] {
                let (mut before, definition) = source(case, retired, profile);
                let mut values = [Some(123); 3];
                for (root, value) in [
                    (0, Some(456)),
                    (0, None),
                    (0, Some(-12)),
                    (0, Some(-12)),
                    (1, None),
                    (2, Some(-42)),
                ] {
                    let expected = updates::number(&before, definition, root, value);
                    let requests = [FixedScalarUpdate::new(
                        root,
                        number_value(values[root]),
                        number_value(value),
                    )];
                    let groups = [FittedColumnUpdates::Fixed {
                        column: "Score",
                        updates: &requests,
                    }];
                    let plan = image(&before)
                        .plan_retained_row_updates("Added", &groups, &mut budget(), &|| false)
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{retired}/{profile:?}/{root}/{value:?}: {error:?}")
                        });
                    equal_image(plan.after_bytes(), &expected);
                    assert_eq!(plan.changed_rows(), usize::from(values[root] != value));
                    values[root] = value;
                    let observed = read_table(
                        &mut Cursor::new(plan.after_bytes()),
                        SCOPE,
                        "Added",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    for (row, value) in observed.rows().iter().zip(values) {
                        assert_eq!(row[1], number_value(value));
                    }
                    before = expected;
                }
            }
        }
    }
}

#[test]
fn retained_text_cells_preserve_forwarded_and_opaque_null_history() {
    let growth = "a longer replacement ".repeat(6);
    let wide = "日本語🦀".repeat(30);
    for case in CASES {
        for retired in [false, true] {
            for profile in [Profile::NativeText, Profile::DesktopText] {
                let (mut before, definition) = source(case, retired, profile);
                let mut values = [Some("initial"); 3];
                let actions = [
                    (0, Some("a")),
                    (0, Some(growth.as_str())),
                    (0, Some(wide.as_str())),
                    (0, None),
                    (0, None),
                    (1, Some("other")),
                    (2, Some(growth.as_str())),
                ];
                for (root, value) in actions {
                    let expected = updates::text(&before, definition, root, value);
                    let requests = [SlotScalarUpdate::new(
                        root,
                        text_value(values[root]),
                        text_value(value),
                    )];
                    let groups = [FittedColumnUpdates::Variable {
                        column: "Name",
                        updates: &requests,
                    }];
                    let plan = image(&before)
                        .plan_retained_row_updates("Added", &groups, &mut budget(), &|| false)
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{retired}/{profile:?}/{root}: {error:?}")
                        });
                    equal_image(plan.after_bytes(), &expected);
                    assert_eq!(plan.changed_rows(), usize::from(values[root] != value));
                    values[root] = value;
                    before = expected;
                }
                if matches!(profile, Profile::DesktopText) {
                    let expected = updates::text(&before, definition, 1, Some(""));
                    let requests = [SlotScalarUpdate::new(
                        1,
                        text_value(values[1]),
                        Value::Text(String::new()),
                    )];
                    let groups = [FittedColumnUpdates::Variable {
                        column: "Name",
                        updates: &requests,
                    }];
                    let plan = image(&before)
                        .plan_retained_row_updates("Added", &groups, &mut budget(), &|| false)
                        .unwrap();
                    equal_image(plan.after_bytes(), &expected);
                }
            }
        }
    }

    let (before, definition) = source(Case::TextPresent, true, Profile::DesktopText);
    let before = updates::with_opaque_text_null(&before, definition, 0);
    let requests = [SlotScalarUpdate::new(0, Value::Null, Value::Null)];
    let groups = [FittedColumnUpdates::Variable {
        column: "Name",
        updates: &requests,
    }];
    let plan = image(&before)
        .plan_retained_row_updates("Added", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.changed_rows(), 0);
    equal_image(plan.after_bytes(), &before);
}

#[test]
fn retained_cells_keep_original_value_conflicts_and_original_sparse_table_identity() {
    for case in CASES {
        for target in [Field::Number, Field::ShortText] {
            let original = readdition::after(&readdition::source(case), target);
            let (original, original_after, expected, value) = match target {
                Field::Number => {
                    let after = readdition::fill_number(case, &original, 0, 456);
                    (original, after, Value::Null, Value::Int32(456))
                }
                Field::ShortText => {
                    let before = readdition::fill_text(case, &original, 0, "old");
                    let after =
                        readdition::replace_text(&before, 0, Some("a much longer replacement"));
                    (
                        before,
                        after,
                        Value::Text("old".into()),
                        Value::Text("a much longer replacement".into()),
                    )
                }
            };
            let before = raw::append(&original, "More", Target::GeneratedNumber);
            let expected_bytes = raw::append(&original_after, "More", Target::GeneratedNumber);
            let fixed = [FixedScalarUpdate::new(0, expected, value)];
            let text = [SlotScalarUpdate::new(
                0,
                Value::Text("old".into()),
                Value::Text("a much longer replacement".into()),
            )];
            let groups = [match target {
                Field::Number => FittedColumnUpdates::Fixed {
                    column: target.name(),
                    updates: &fixed,
                },
                Field::ShortText => FittedColumnUpdates::Variable {
                    column: target.name(),
                    updates: &text,
                },
            }];
            let plan = image(&before)
                .plan_retained_row_updates("Table1", &groups, &mut budget(), &|| false)
                .unwrap();
            equal_image(plan.after_bytes(), &expected_bytes);
            let schema = inspect_table_schema(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(schema.definition_page(), 7);
        }
    }
    let (before, _) = source(Case::NumberPresent, true, Profile::DesktopNumber);
    let conflict = [FixedScalarUpdate::new(
        0,
        Value::Int32(999),
        Value::Int32(456),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Score",
        updates: &conflict,
    }];
    assert!(matches!(
        image(&before).plan_retained_row_updates("Added", &groups, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));
    assert_eq!(image(&before).as_bytes(), before);
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

#[test]
fn retained_updates_follow_paid_long_user_and_catalog_chains_without_losing_legacy_guards() {
    let (base, definition) = source(Case::MixedBinary, true, Profile::DesktopNumber);
    let native = image(&base);
    let schema = inspect_table_schema(&mut native.reader(), SCOPE, "Added", &mut budget(), &|| {
        false
    })
    .unwrap();
    let origin = native
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == schema.object_id())
        .unwrap();
    assert_eq!(origin.page(), 4);
    let catalog_root = usize::from(origin.slot());
    let user = raw_forwarding_chain::extend(&base, definition + 1, 0, 20);
    let catalog = raw_forwarding_chain::extend(&base, 4, catalog_root, 20);
    let both = raw_forwarding_chain::extend(&user, 4, catalog_root, 20);
    for (scenario, before) in [("user", &user), ("catalog", &catalog), ("both", &both)] {
        let native = image(before);
        native
            .check_native_retained_rows_table("Added", &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("{scenario}: check: {error:?}"));
        let noop = [FixedScalarUpdate::new(
            0,
            Value::Int32(123),
            Value::Int32(123),
        )];
        let groups = [FittedColumnUpdates::Fixed {
            column: "Score",
            updates: &noop,
        }];
        let plan = image(before)
            .plan_retained_row_updates("Added", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.changed_rows(), 0);
        equal_image(plan.after_bytes(), before);

        let mut expected = before.to_vec();
        let data = (definition as usize + 1) * 4096;
        let root = word(before, data + 14);
        let terminal = if root & 0x4000 == 0 {
            0
        } else {
            word(before, data + 12) - 1
        };
        let at = data + (word(before, data + 14 + terminal * 2) & 0x1fff);
        assert_eq!(word(before, at), 2);
        assert_eq!(&before[at + 6..at + 10], 123_i32.to_le_bytes());
        expected[at + 6..at + 10].copy_from_slice(&456_i32.to_le_bytes());
        let updates = [FixedScalarUpdate::new(
            0,
            Value::Int32(123),
            Value::Int32(456),
        )];
        let groups = [FittedColumnUpdates::Fixed {
            column: "Score",
            updates: &updates,
        }];
        let plan = image(before)
            .plan_retained_row_updates("Added", &groups, &mut budget(), &|| false)
            .unwrap();
        equal_image(plan.after_bytes(), &expected);
        assert_eq!(plan.changed_rows(), 1);
    }

    // The old physical-proof API retains its explicit 16-hop language.
    let native = image(&user);
    let mut source = native.reader();
    let mut work = budget();
    let mut reader = crate::format::reader::Reader {
        source: &mut source,
        header: native.catalog().header(),
        budget: &mut work,
        cancelled: &|| false,
        reads: 0,
    };
    let page = reader.page(definition + 1).unwrap();
    assert!(matches!(
        reader.row_location(definition + 1, 0, definition, &page),
        Err(InspectError::StructureLimit)
    ));

    let data = (definition as usize + 1) * 4096;
    let count = word(&user, data + 12);
    let entry = data + 14 + (count - 1) * 2;
    let terminal = word(&user, entry);
    let at = data + (terminal & 0x1fff);
    let mut cycle = user;
    cycle[entry..entry + 2].copy_from_slice(&((terminal as u16) | 0x4000).to_le_bytes());
    cycle[at..at + 4].copy_from_slice(&((definition + 1) << 8).to_le_bytes());
    let request = [FixedScalarUpdate::new(
        0,
        Value::Int32(123),
        Value::Int32(456),
    )];
    let groups = [FittedColumnUpdates::Fixed {
        column: "Score",
        updates: &request,
    }];
    assert!(matches!(
        image(&cycle).plan_retained_row_updates("Added", &groups, &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Malformed))
    ));
    let mut wrong_owner = both;
    wrong_owner[data + 4..data + 8].copy_from_slice(&7_u32.to_le_bytes());
    assert!(
        image(&wrong_owner)
            .plan_retained_row_updates("Added", &groups, &mut budget(), &|| false)
            .is_err()
    );
}
