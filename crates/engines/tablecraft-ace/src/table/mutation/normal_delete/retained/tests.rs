//! Independent retained root tombstones and unchanged physical history.
use crate::{EditError, NativeImage, inspect_table_schema};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_delete,
        deleted_field::populated::{CASES, Case, readdition::Target as Field},
    },
    canonical_retained_add_table::{
        self as raw, Target, deletes, desktop,
        rows::{self, Input, Profile},
        updates,
    },
    canonical_table_drop::current::{self as dropped, History},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(3803);

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
        actual.iter().zip(expected).position(|(a, b)| a != b)
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
        _ => Target::GeneratedText,
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
    bytes = match profile {
        Profile::NativeNumber | Profile::DesktopNumber => {
            updates::number(&bytes, definition, 0, None)
        }
        _ => updates::text(
            &bytes,
            definition,
            0,
            Some(&"forwarded replacement ".repeat(8)),
        ),
    };
    (bytes, definition)
}

fn check_delete(before: &[u8], table: &str, ordinals: &[usize], expected: &[u8]) {
    let native = image(before);
    let requests = native
        .prepare_retained_row_deletes(table, ordinals, &mut budget(), &|| false)
        .unwrap();
    let plan = native
        .plan_retained_row_deletes(table, &requests, &mut budget(), &|| false)
        .unwrap();
    equal_image(plan.before_bytes(), before);
    equal_image(plan.after_bytes(), expected);
    assert_eq!(plan.changed_rows(), ordinals.len());
    image(expected)
        .check_native_retained_delete_table(table, &mut budget(), &|| false)
        .unwrap();
}

#[test]
fn retained_delete_matches_raw_subsets_after_cell_updates_and_retired_tables() {
    for case in CASES {
        for retired in [false, true] {
            for profile in [
                Profile::NativeNumber,
                Profile::NativeText,
                Profile::DesktopNumber,
                Profile::DesktopText,
            ] {
                let (before, definition) = source(case, retired, profile);
                for selected in [
                    vec![],
                    vec![0],
                    vec![1],
                    vec![2],
                    vec![0, 1],
                    vec![0, 2],
                    vec![1, 2],
                    vec![0, 1, 2],
                ] {
                    let requests = image(&before)
                        .prepare_retained_row_deletes("Added", &selected, &mut budget(), &|| false)
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{retired}/{profile:?}/{selected:?}: {error:?}")
                        });
                    for (request, &root) in requests.iter().zip(&selected) {
                        assert_eq!(
                            request.expected_bytes(),
                            deletes::record(&before, definition, root)
                        );
                    }
                    let expected = deletes::after(&before, definition, &selected);
                    check_delete(&before, "Added", &selected, &expected);
                }
                let expected = deletes::after(&before, definition, &[0, 2]);
                check_delete(&before, "Added", &[2, 0], &expected);
                let empty = deletes::after(&before, definition, &[0, 1, 2]);
                let physical = usize::from(u16::from_le_bytes(
                    empty[(definition as usize + 1) * 4096 + 12
                        ..(definition as usize + 1) * 4096 + 14]
                        .try_into()
                        .unwrap(),
                ));
                let reinserted = rows::append(&empty, definition, profile, Input::Omitted);
                let expected = deletes::after(&reinserted, definition, &[physical]);
                check_delete(&reinserted, "Added", &[0], &expected);
            }
        }
    }
}

#[test]
fn retained_delete_original_table_commutes_with_sibling_creation() {
    for case in CASES {
        for target in [Field::Number, Field::ShortText] {
            let source = current_delete::source(case, target, true, true, true);
            let before = raw::append(&source.bytes, "More", Target::GeneratedNumber);
            let selected = [0, source.roots.len() - 1];
            let roots = selected.map(|ordinal| source.roots[ordinal]);
            let deleted = current_delete::after(&source.bytes, &roots);
            let expected = raw::append(&deleted, "More", Target::GeneratedNumber);
            check_delete(&before, "Table1", &selected, &expected);
        }
    }
}

#[test]
fn retained_delete_follows_long_user_and_catalog_chains_and_rejects_corruption() {
    let (base, definition) = source(Case::MixedBinary, true, Profile::DesktopText);
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
    let user = raw_forwarding_chain::extend(&base, definition + 1, 0, 20);
    let catalog = raw_forwarding_chain::extend(&base, 4, usize::from(origin.slot()), 20);
    let both = raw_forwarding_chain::extend(&user, 4, usize::from(origin.slot()), 20);
    for (scenario, before) in [("user", &user), ("catalog", &catalog), ("both", &both)] {
        image(before)
            .check_native_retained_delete_table("Added", &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("{scenario}: source: {error:?}"));
        for selected in [vec![], vec![0], vec![0, 1, 2]] {
            let expected = deletes::after(before, definition, &selected);
            check_delete(before, "Added", &selected, &expected);
        }
    }
    let data = (definition as usize + 1) * 4096;
    let count = usize::from(u16::from_le_bytes(
        user[data + 12..data + 14].try_into().unwrap(),
    ));
    let entry = data + 14 + (count - 1) * 2;
    let terminal = u16::from_le_bytes(user[entry..entry + 2].try_into().unwrap());
    let at = data + usize::from(terminal & 0x1fff);
    let mut cycle = user;
    cycle[entry..entry + 2].copy_from_slice(&(terminal | 0x4000).to_le_bytes());
    cycle[at..at + 4].copy_from_slice(&((definition + 1) << 8).to_le_bytes());
    assert!(
        image(&cycle)
            .prepare_retained_row_deletes("Added", &[], &mut budget(), &|| false)
            .is_err()
    );
    let mut wrong_owner = both;
    wrong_owner[data + 4..data + 8].copy_from_slice(&7_u32.to_le_bytes());
    assert!(
        image(&wrong_owner)
            .prepare_retained_row_deletes("Added", &[], &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn retained_delete_rechecks_witnesses_selection_and_control_failures() {
    let (before, _) = source(Case::NumberPresent, true, Profile::DesktopNumber);
    for ordinals in [vec![0, 0], vec![usize::MAX], vec![3]] {
        assert!(
            image(&before)
                .prepare_retained_row_deletes("Added", &ordinals, &mut budget(), &|| false)
                .is_err()
        );
    }
    assert!(
        image(&before)
            .prepare_retained_row_deletes("Added", &[], &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_retained_row_deletes("Added", &[], &mut ResourceBudget::new(0, 0, 0), &|| {
                false
            })
            .is_err()
    );
    let request = image(&before)
        .prepare_retained_row_deletes("Added", &[0], &mut budget(), &|| false)
        .unwrap();
    let mut stale = before;
    let at = stale
        .windows(request[0].expected_bytes().len())
        .position(|window| window == request[0].expected_bytes())
        .unwrap();
    stale[at + 6] ^= 1;
    assert!(matches!(
        image(&stale).plan_retained_row_deletes("Added", &request, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));
}
