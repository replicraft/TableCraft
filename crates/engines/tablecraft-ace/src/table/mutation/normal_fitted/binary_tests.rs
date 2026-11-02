//! Source-qualified fitting Binary updates and independent whole-image proofs.
use super::indexed::proof;
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeFittedEditIntent,
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeIntentError,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError, SlotScalarUpdate, WriteBlocker,
    read_table,
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_binary_update::{self as source, Case};

const SCOPE: CatalogScope = CatalogScope::new(217);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        );
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

fn requests(case: &Case, reverse: bool) -> Vec<NativeFittedIntentGroup> {
    let mut updates = source::updates(case.recipe)
        .into_iter()
        .map(|(row, expected, value)| SlotScalarUpdate::new(row, expected, value))
        .collect::<Vec<_>>();
    if reverse {
        updates.reverse();
    }
    vec![
        NativeFittedIntentGroup::new(
            "KeyValue",
            NativeFittedUpdates::Variable(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn prove(case: &Case, plan: &NativeEditPlan) -> Result<(), crate::EditError> {
    let original = NativeFittedEditIntent::new(
        "BinaryKeys",
        requests(case, false),
        &case.before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let groups = original.borrowed_groups(&mut budget(), &|| false).unwrap();
    proof::prove(plan, SCOPE, "BinaryKeys", &groups, &mut budget(), &|| false)
}

#[test]
#[ignore = "requires 16 qualified Binary UPDATE source views"]
fn binary_update_public_fitting_and_auto_plans_match_literals_with_original_tcni5() {
    let mut plans = 0;
    for case in cases() {
        for reverse in [false, true] {
            if reverse && case.recipe != "duplicate-merge" {
                continue;
            }
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "BinaryKeys",
                    requests(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.changed_rows(), case.changed_rows());
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFitted
            );
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(&wire[..8], b"TCNI0005");
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
            let groups = decoded
                .fitted()
                .unwrap()
                .borrowed_groups(&mut budget(), &|| false)
                .unwrap();
            let mut expected = source::updates(case.recipe);
            if reverse {
                expected.reverse();
            }
            for (position, (row, old, new)) in expected.iter().enumerate() {
                let actual = groups[0].cell(position).unwrap();
                assert_eq!(
                    (actual.row, actual.expected, actual.value),
                    (*row, old, new)
                );
            }
            let auto = image(&case.before)
                .plan_recoverable_ordinary_variable_updates(
                    "BinaryKeys",
                    requests(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                auto.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFitted
            );
            assert_eq!(auto.after_bytes(), case.after);
            plans += 1;
        }
    }
    assert_eq!(plans, 16);
}

#[test]
#[ignore = "requires 16 qualified Binary UPDATE source views"]
fn binary_literal_proof_rejects_data_mask_capacity_history_row_and_protected_changes() {
    let mut accepted = 0;
    let mut refused = 0;
    for case in cases() {
        let literal = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed_rows(),
        };
        prove(&case, &literal).unwrap();
        accepted += 1;
        let root = case.roots[1] as usize * 4096;
        let descending = case.roots[2] as usize * 4096;
        let data = case.data as usize * 4096;
        let td = case.definition as usize * 4096;
        // Literal source geometry: untouched sibling, mask, variable footer,
        // DATA free, history, map, active ROW, bitmap and inactive root tail.
        for at in [
            data + 4085,
            data + 4095,
            data + 4089,
            data + 2,
            td + 67 + 12,
            4096 + 100,
            root + 484,
            root + 27,
            root + 4000,
            descending + 490,
        ] {
            let mut forged = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: case.changed_rows(),
            };
            forged.after[at] ^= 1;
            assert!(prove(&case, &forged).is_err(), "{} byte{at}", case.recipe);
            refused += 1;
        }
        let wrong_count = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        assert!(prove(&case, &wrong_count).is_err());
        refused += 1;
    }
    assert_eq!((accepted, refused), (14, 154));
}

#[test]
#[ignore = "requires 16 qualified Binary UPDATE source views"]
fn binary_empty_noop_stale_and_declared_fitting_capacity_are_checked_before_publication() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "duplicate-to-new")
    {
        assert!(
            image(&case.before)
                .select_native_ordinary_fitted_table("BinaryKeys", &mut budget(), &|| false)
                .unwrap()
        );
        image(&case.before)
            .check_native_ordinary_fitted_table("BinaryKeys", &mut budget(), &|| false)
            .unwrap();
        for updates in [
            vec![],
            vec![SlotScalarUpdate::new(0, Value::Null, Value::Null)],
        ] {
            let original = vec![
                NativeFittedIntentGroup::new(
                    "KeyValue",
                    NativeFittedUpdates::Variable(updates),
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            ];
            let plan = image(&case.before)
                .plan_recoverable_ordinary_variable_updates(
                    "BinaryKeys",
                    original,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), case.before);
            assert_eq!(plan.changed_rows(), 0);
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFitted
            );
        }
        for update in [
            SlotScalarUpdate::new(2, Value::Null, Value::Binary(vec![85])),
            SlotScalarUpdate::new(2, Value::Binary(vec![0]), Value::Binary(vec![1, 2])),
        ] {
            let original = vec![
                NativeFittedIntentGroup::new(
                    "KeyValue",
                    NativeFittedUpdates::Variable(vec![update]),
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            ];
            assert!(
                image(&case.before)
                    .plan_recoverable_ordinary_variable_updates(
                        "BinaryKeys",
                        original,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
        let empty = FittedColumnUpdates::Variable {
            column: "KeyValue",
            updates: &[],
        };
        super::indexed::preflight(
            &image(&case.before),
            "BinaryKeys",
            &[empty],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}

#[test]
#[ignore = "requires 16 qualified Binary UPDATE source views"]
fn binary_public_planning_has_exact_pools_and_cancellable_complete_proof() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "duplicate-merge")
    {
        let mut measured = budget();
        let start = POOLS.map(|pool| measured.remaining(pool));
        image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "BinaryKeys",
                requests(&case, false),
                &mut measured,
                &|| false,
            )
            .unwrap();
        let costs = std::array::from_fn::<_, 3, _>(|i| start[i] - measured.remaining(POOLS[i]));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "BinaryKeys",
                requests(&case, false),
                &mut exact,
                &|| false,
            )
            .unwrap();
        for pool in POOLS {
            assert_eq!(exact.remaining(pool), 0);
        }
        for index in 0..3 {
            let mut limits = costs;
            limits[index] -= 1;
            let mut short = ResourceBudget::new(limits[0], limits[1], limits[2]);
            let error = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "BinaryKeys",
                    requests(&case, false),
                    &mut short,
                    &|| false,
                )
                .err()
                .expect("one-less work cannot return a plan");
            assert!(
                matches!(inspection(error), InspectError::Limit(limit) if limit.resource == POOLS[index])
            );
        }
        let trace = Cell::new(0);
        image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "BinaryKeys",
                requests(&case, false),
                &mut budget(),
                &|| {
                    trace.set(trace.get() + 1);
                    false
                },
            )
            .unwrap();
        for stop in [1, trace.get() / 2, trace.get()] {
            let calls = Cell::new(0);
            let error = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "BinaryKeys",
                    requests(&case, false),
                    &mut budget(),
                    &|| {
                        calls.set(calls.get() + 1);
                        calls.get() == stop
                    },
                )
                .err()
                .expect("cancelled work cannot return a plan");
            assert_eq!(inspection(error), InspectError::Cancelled);
            assert_eq!(calls.get(), stop);
        }
    }
}

#[test]
#[ignore = "requires 16 qualified fitting Binary UPDATE source views"]
fn binary_update_empty_and_nonempty_requests_reject_readable_unknown_and_noncanonical_metadata() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "duplicate-to-new")
    {
        let td = case.definition as usize * 4096;
        let mut unknown = case.before.clone();
        // Three physical count records, then three 25-byte field descriptors.
        // The reader ignores this descriptor signature byte; raw Source does not.
        let binary = td + 63 + 3 * 12 + 25;
        assert_eq!(unknown[binary + 1], 0x59);
        unknown[binary + 1] = 0x58;
        let mut stored_id = case.before.clone();
        let marker_id = td + 63 + 3 * 12 + 2 * 25 + 9;
        assert_eq!(&stored_id[marker_id..marker_id + 2], &[2, 0]);
        stored_id[marker_id..marker_id + 2].copy_from_slice(&7_u16.to_le_bytes());
        for raw in [unknown, stored_id] {
            let rows = read_table(
                &mut Cursor::new(&raw),
                SCOPE,
                "BinaryKeys",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                rows.rows(),
                tablecraft_testkit::fixtures::access::genuine_binary_indexed::original_rows()
            );
            for empty in [false, true] {
                let groups = if empty {
                    vec![
                        NativeFittedIntentGroup::new(
                            "KeyValue",
                            NativeFittedUpdates::Variable(vec![]),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap(),
                    ]
                } else {
                    requests(&case, false)
                };
                assert!(matches!(
                    image(&raw).plan_recoverable_ordinary_fitted_updates(
                        "BinaryKeys",
                        groups,
                        &mut budget(),
                        &|| false
                    ),
                    Err(RecoverableEditError::Edit(EditError::Unsupported(
                        WriteBlocker::ActiveContent
                    )))
                ));
            }
        }
    }
}

fn inspection(error: RecoverableEditError) -> InspectError {
    match error {
        RecoverableEditError::Edit(EditError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Request(EditError::Inspect(error))) => {
            error
        }
        other => panic!("unexpected resource refusal: {other:?}"),
    }
}

#[test]
#[ignore = "requires 16 qualified fitting Binary UPDATE source views"]
fn stale_binary_physical_comparison_pays_its_own_payload_pass_before_conflict() {
    use crate::table::indexes::mutation::normal_binary_update::{self, Change};

    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "duplicate-to-new")
    {
        let image = image(&case.before);
        let indexes = crate::inspect_table_indexes(
            &mut image.reader(),
            SCOPE,
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let index = indexes
            .indexes()
            .iter()
            .find(|index| index.name() == "Ascending")
            .unwrap();
        let mut empty = budget();
        normal_binary_update::patches(&image, indexes.schema(), index, &[], &mut empty, &|| false)
            .unwrap();

        let wrong = Value::Binary(vec![1]);
        let value = Value::Null;
        let changes = [Change {
            reference: case.data << 8,
            expected: &wrong,
            value: &value,
        }];
        let mut stale = budget();
        assert_eq!(
            normal_binary_update::patches(
                &image,
                indexes.schema(),
                index,
                &changes,
                &mut stale,
                &|| false
            )
            .err()
            .unwrap(),
            EditError::Conflict
        );
        // Source has 13 entries and a declared 16-byte Binary field. The common
        // page-read work cancels: stopping at the first (NULL) entry saves 12
        // row visits, performs one ROW lookup, and prepays two 16-byte passes.
        assert_eq!(
            empty.remaining(Resource::Steps) - stale.remaining(Resource::Steps),
            32 + 1 - 12
        );
        let cost = 1_000_000_000 - stale.remaining(Resource::Steps);
        let mut short = ResourceBudget::new(1_000_000_000, 1_000_000, cost - 1);
        assert!(
            matches!(normal_binary_update::patches(&image, indexes.schema(), index, &changes, &mut short, &|| false), Err(EditError::Inspect(InspectError::Limit(limit))) if limit.resource == Resource::Steps)
        );
    }
}
