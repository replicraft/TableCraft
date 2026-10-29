//! Correlated staging must not substitute its own request/counter authority.
use crate::table::mutation::fixed::batch::split_tests::model::{self, Case, Shape};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, NativeImage, TableCheck,
    check_table, inspect_table_index_counter,
    table::{
        indexes::{IndexDataPatch, SplitIndexPlan, fixed_batch_split_patches},
        mutation::{fixed::masked, gates, index_append, patch::spans},
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn inputs(case: &Case) -> [Vec<FixedScalarUpdate>; 2] {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (ordinal, (before, after)) in case.original.iter().zip(&case.final_rows).enumerate() {
        left.push(FixedScalarUpdate::new(
            ordinal,
            before.left.map_or(Value::Null, Value::Int32),
            after.left.map_or(Value::Null, Value::Int32),
        ));
        right.push(FixedScalarUpdate::new(
            ordinal,
            before.right.map_or(Value::Null, Value::Int32),
            after.right.map_or(Value::Null, Value::Int32),
        ));
    }
    [left, right]
}

fn qualify(bytes: &[u8]) {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
}

fn stage(
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
) -> (TableCheck, SplitIndexPlan, Vec<u8>) {
    let checked = gates::before_fixed_update(image, "Items", &mut budget(), &|| false).unwrap();
    let mut fields = Vec::new();
    let mut keys = Vec::new();
    for group in groups {
        let staged = masked::stage::<4, true>(
            image,
            &checked,
            group.column(),
            group.updates(),
            ScalarType::Int32,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for patch in &staged.patches {
            spans::push(&mut fields, patch, &mut budget(), &|| false).unwrap();
        }
        keys.extend(staged.keys);
    }
    spans::coalesce(&mut fields, &mut budget(), &|| false).unwrap();
    let plan = fixed_batch_split_patches(
        image,
        checked.schema(),
        checked.indexes(),
        &keys,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let after = index_append::compose(image, &fields, &plan, &mut budget(), &|| false).unwrap();
    (checked, plan, after)
}

fn equal(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first differing byte: {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
    );
}

#[test]
fn original_groups_reject_a_correlated_alternative_valid_population() {
    for code in [5, 6] {
        let case = model::source(code, Shape::Empty, false, false, false);
        qualify(&case.before);
        qualify(&case.expected);
        let [left, right] = inputs(&case);
        let groups = [
            FixedColumnUpdates::new("Items", "Group", &left),
            FixedColumnUpdates::new("Items", "Score", &right),
        ];
        let image = capture(&case.before);
        let (checked, plan, after) = stage(&image, &groups);
        equal(&after, &case.expected);
        assert_eq!(
            super::prove(
                &image,
                &[&checked],
                &groups,
                &plan,
                &after,
                &mut budget(),
                &|| false
            )
            .unwrap(),
            500
        );

        let [mut altered, same_right] = inputs(&case);
        let Some(value) = case.final_rows[0].left else {
            panic!("literal value required")
        };
        altered[0] =
            FixedScalarUpdate::new(0, Value::Null, Value::Int32(value.checked_add(1).unwrap()));
        let alternative = [
            FixedColumnUpdates::new("Items", "Group", &altered),
            FixedColumnUpdates::new("Items", "Score", &same_right),
        ];
        let (checked, forged_plan, forged_after) = stage(&image, &alternative);
        qualify(&forged_after);
        assert!(
            super::prove(
                &image,
                &[&checked],
                &alternative,
                &forged_plan,
                &forged_after,
                &mut budget(),
                &|| false
            )
            .is_ok()
        );
        assert_eq!(
            super::prove(
                &image,
                &[&checked],
                &groups,
                &forged_plan,
                &forged_after,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn candidate_and_staged_counter_agreement_does_not_authorize_extra_history() {
    for code in [5, 6] {
        let case = model::source(code, Shape::Empty, false, false, false);
        qualify(&case.before);
        qualify(&case.expected);
        let [left, right] = inputs(&case);
        let groups = [
            FixedColumnUpdates::new("Items", "Group", &left),
            FixedColumnUpdates::new("Items", "Score", &right),
        ];
        let image = capture(&case.before);
        let (checked, mut plan, mut after) = stage(&image, &groups);
        equal(&after, &case.expected);
        assert!(
            super::prove(
                &image,
                &[&checked],
                &groups,
                &plan,
                &after,
                &mut budget(),
                &|| false
            )
            .is_ok()
        );
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            "PairOrder",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let range = counter.byte_range();
        let mut current = [0; 4];
        current.copy_from_slice(&after[range.clone()]);
        let wrong = u32::from_le_bytes(current)
            .checked_add(1)
            .unwrap()
            .to_le_bytes();
        after[range.clone()].copy_from_slice(&wrong);
        let patch = plan
            .patches
            .iter_mut()
            .find(|patch| patch.offset == range.start)
            .unwrap();
        patch.after.copy_from_slice(&wrong);
        qualify(&after);
        assert_eq!(
            super::prove(
                &image,
                &[&checked],
                &groups,
                &plan,
                &after,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn unreachable_owned_index_page_never_grants_correlated_patch_authority() {
    use crate::table::mutation::fixed::batch::split_tests::orphans;
    for code in [5, 6] {
        let sources = orphans::sources(code);
        qualify(&sources.good);
        qualify(&sources.bad);
        let update = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(7))];
        let groups = [FixedColumnUpdates::new("Items", "Group", &update)];
        let image = capture(&sources.good);
        let (checked, mut plan, mut after) = stage(&image, &groups);
        assert_eq!(after.len(), sources.good.len());
        assert!(
            super::prove(
                &image,
                &[&checked],
                &groups,
                &plan,
                &after,
                &mut budget(),
                &|| false
            )
            .is_ok()
        );
        for (offset, (&good, &bad)) in sources.good.iter().zip(&sources.bad).enumerate() {
            if good != bad {
                after[offset] = bad;
            }
        }
        let offset = 18 * 4096 + 4095;
        let before = after[offset];
        after[offset] ^= 0x40;
        plan.patches.push(IndexDataPatch {
            offset,
            before: vec![before],
            after: vec![after[offset]],
        });
        qualify(&after);
        let bad = capture(&sources.bad);
        let checked = gates::before_fixed_update(&bad, "Items", &mut budget(), &|| false).unwrap();
        assert_eq!(
            super::prove(
                &bad,
                &[&checked],
                &groups,
                &plan,
                &after,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Preservation)
        );
    }
}
