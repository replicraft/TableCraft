//! Real normal TCNI4 omitted-key UPDATE over held genuine single-key originals.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::{io::Cursor, ops::Bound};
use tablecraft_ace::{
    EditError, FixedScalarUpdate, IndexConstraintViolation, InspectError, Int32IndexSelection,
    NativeFixedIntentGroup, NativeImage, NativeUpdateIntent, RecoverableEditError, WriteBlocker,
    read_table_index, read_table_index_range,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_ignore_null_update::{
    self as source, Case, Recipe, Refusal,
};

#[path = "ignore_null_update/recovery.rs"]
mod recovery;
#[path = "ignore_null_update/resources.rs"]
mod resources;

fn all() -> Vec<Case> {
    source::load_all(|raw, expected| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn request(updates: Vec<(usize, Value, Value)>) -> Vec<NativeFixedIntentGroup> {
    let updates = updates
        .into_iter()
        .map(|(row, before, after)| FixedScalarUpdate::new(row, before, after))
        .collect();
    vec![NativeFixedIntentGroup::new("Left", updates, &mut budget(), &|| false).unwrap()]
}

fn wire(case: &Case) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    case.wire(digest.finalize().into())
}

fn expected_refusal(refusal: Refusal) -> EditError {
    match refusal {
        Refusal::Required => EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::NullKey,
        )),
        Refusal::Unique => EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey,
        )),
        _ => EditError::Unsupported(WriteBlocker::IndexedColumn),
    }
}

#[test]
#[ignore = "requires the two genuine IgnoreNULL originals and six literal AFTERs"]
fn single_omission_changes_and_three_distinct_empty_effect_requests_commit_exact_images() {
    let cases = all();
    assert_eq!(cases.len(), 12);
    let mut commits = 0;
    for case in &cases {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates(
                "Single",
                request(case.updates()),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), case.changed());
        let literal = wire(case);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal
        );
        let decoded = NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), literal);
        let mut target = Storage::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        let count = plan
            .prepare(&mut target, &mut log, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(count, case.changed());
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            log.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, journals::Marker::Committed)
        );
        let saved = image(target.bytes.get_ref());
        for alias in ["PrimaryID", "PrimaryAlias", "ValueOrder", "ValueAlias"] {
            let selected = if matches!(alias, "ValueOrder" | "ValueAlias") {
                // This is the complete non-NULL key domain, not an All table
                // snapshot. IgnoreNULL never grants the missing NULL rows.
                read_table_index_range(
                    &mut saved.reader(),
                    scope(),
                    "Single",
                    alias,
                    Int32IndexSelection::Range {
                        lower: Bound::Unbounded,
                        upper: Bound::Unbounded,
                    },
                    &mut budget(),
                    &|| false,
                )
            } else {
                read_table_index(
                    &mut saved.reader(),
                    scope(),
                    "Single",
                    alias,
                    &mut budget(),
                    &|| false,
                )
            }
            .unwrap();
            assert_eq!(selected.rows(), case.index_rows(alias));
        }
        case.assert_preserved();
        commits += 1;
    }
    assert_eq!(commits, 12);
}

#[test]
#[ignore = "requires the existing genuine single-key source declarations"]
fn required_unique_and_both_historical_family_orders_refuse_before_a_plan_exists() {
    let mut refusals = 0;
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullNew)
    {
        for refusal in source::REFUSALS {
            let error = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    refusal.table(),
                    request(refusal.updates()),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert_eq!(error, RecoverableEditError::Edit(expected_refusal(refusal)));
            refusals += 1;
        }
    }
    // Four historical ambiguous vectors already contain both written orders.
    assert_eq!(refusals, 12);
}

#[test]
#[ignore = "requires the genuine IgnoreNULL originals"]
fn stale_logical_null_and_exact_carriers_cannot_reinterpret_inactive_payload() {
    let mut refusals = 0;
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullNew)
    {
        for (row, before, after, expected) in [
            (0, Value::Int32(11), Value::Int32(30), EditError::Conflict),
            (4, Value::Int32(10), Value::Int32(30), EditError::Conflict),
            // Original ROW2 is NULL, although its inactive native payload is 10.
            (2, Value::Int32(10), Value::Int32(30), EditError::Conflict),
            (
                0,
                Value::Int64(10),
                Value::Int32(30),
                EditError::Unsupported(WriteBlocker::Column),
            ),
            (
                0,
                Value::Int32(10),
                Value::Int64(30),
                EditError::Unsupported(WriteBlocker::Column),
            ),
        ] {
            assert_eq!(
                image(&case.before)
                    .plan_recoverable_ordinary_fixed_updates(
                        "Single",
                        request(vec![(row, before, after)]),
                        &mut budget(),
                        &|| false,
                    )
                    .err(),
                Some(RecoverableEditError::Edit(expected))
            );
            refusals += 1;
        }
    }
    assert_eq!(refusals, 10);
}

#[test]
#[ignore = "requires genuine IgnoreNULL originals; counter MAX is an adversarial variant"]
fn historical_counter_max_distinguishes_zero_delta_from_new_family_overflow() {
    let mut controls = [0; 2];
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullExisting)
    {
        // This variant changes only the literal Single secondary counter. It is
        // not a newly qualified source/golden or a producer-generated oracle.
        let counter = case.definition as usize * 4096 + 79;
        assert_eq!(&case.before[counter..counter + 4], &2_u32.to_le_bytes());
        assert_eq!(&case.after[counter..counter + 4], &2_u32.to_le_bytes());
        let mut before = case.before.clone();
        let mut expected = case.after.clone();
        before[counter..counter + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        expected[counter..counter + 4].copy_from_slice(&u32::MAX.to_le_bytes());

        // Joining the original family emits a ROW but consumes no history slot.
        let plan = image(&before)
            .plan_recoverable_ordinary_fixed_updates(
                "Single",
                request(case.updates()),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);
        controls[0] += 1;

        // The same original NULL introduces a genuinely new family instead.
        assert_eq!(
            image(&before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Single",
                    request(vec![(2, Value::Null, Value::Int32(30))]),
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(RecoverableEditError::Edit(EditError::Inspect(
                InspectError::StructureLimit
            ))),
        );
        controls[1] += 1;
    }
    assert_eq!(controls, [2, 2]);
}

#[test]
#[ignore = "requires genuine IgnoreNULL originals with unindexed Marker"]
fn unindexed_marker_mutation_preserves_every_retained_ignored_owner_byte() {
    let mut controls = 0;
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullNew)
    {
        // The independently captured fixed record is 19 bytes: count, four I32
        // payloads, then presence mask. Marker is the fourth payload at byte 14.
        let record = case.data as usize * 4096 + 4096 - 19;
        assert_eq!(&case.before[record..record + 2], &[4, 0]);
        assert_eq!(
            &case.before[record + 14..record + 18],
            &1001_i32.to_le_bytes()
        );
        assert_eq!(case.before[record + 18] & 8, 8);
        let mut expected = case.before.clone();
        expected[record + 14..record + 18].copy_from_slice(&1005_i32.to_le_bytes());
        let updates = vec![FixedScalarUpdate::new(
            0,
            Value::Int32(1001),
            Value::Int32(1005),
        )];
        let group =
            NativeFixedIntentGroup::new("Marker", updates, &mut budget(), &|| false).unwrap();
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates("Single", vec![group], &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);
        // Whole-image equality above protects both physical owners, aliases,
        // historical counters, maps, unselected NULL payloads and original EOF.
        controls += 1;
    }
    assert_eq!(controls, 2);
}
