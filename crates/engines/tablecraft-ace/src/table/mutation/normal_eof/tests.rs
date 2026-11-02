//! Direct physical EOF planning against independently qualified complete images.
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeFittedIntentGroup,
    NativeFittedUpdates, NativeImage, NativeUpdateProfile, SlotScalarUpdate, WriteBlocker,
    read_table,
};
use sha2::{Digest, Sha256};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_eof_allocation::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(151);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

fn updates(replacement: &str) -> Vec<SlotScalarUpdate> {
    vec![SlotScalarUpdate::new(
        0,
        Value::Text(sources::OLD.into()),
        Value::Text(replacement.into()),
    )]
}

fn plan(
    raw: &[u8],
    updates: &[SlotScalarUpdate],
    work: &mut ResourceBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    // Input capture is separate. The entire selected physical planner, including
    // fresh Source, map ownership, staging and independent proof, uses work.
    image(raw).plan_ordinary_eof_updates(
        "Sentinel",
        &[FittedColumnUpdates::Variable {
            column: "Marker",
            updates,
        }],
        work,
        cancel,
    )
}

fn inspection(error: EditError) -> InspectError {
    match error {
        EditError::Inspect(error) => error,
        error => panic!("expected typed resource/cancellation error, got {error:?}"),
    }
}

fn readable(raw: &[u8]) {
    let snapshot = read_table(
        &mut Cursor::new(raw),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        snapshot.rows(),
        [vec![
            Value::Int32(23),
            Value::Text(sources::OLD.into()),
            Value::Binary(vec![199, 0, 255])
        ]]
    );
}

#[test]
#[ignore = "requires all four hash-pinned EOF source images"]
fn eof_complete_physical_plans_pay_exact_pools_and_every_cancellation_boundary() {
    for case in cases() {
        let original = case.before.clone();
        let request = updates(sources::GROWN);
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let actual = plan(&case.before, &request, &mut measured, &|| false).unwrap();
        assert_eq!(actual.before_bytes(), case.before);
        assert_eq!(actual.after_bytes(), case.after);
        assert_eq!(actual.changed_rows(), 1);
        let costs: [u64; 3] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        let result = plan(&case.before, &request, &mut exact, &|| false).unwrap();
        assert_eq!(result.after_bytes(), case.after);
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
        for index in 0..3 {
            let mut below = costs;
            below[index] -= 1;
            let error = plan(
                &case.before,
                &request,
                &mut ResourceBudget::new(below[0], below[1], below[2]),
                &|| false,
            )
            .err()
            .expect("one-less pool cannot publish a candidate");
            assert!(
                matches!(inspection(error), InspectError::Limit(error) if error.resource == POOLS[index])
            );
        }
        let polls = Cell::new(0_usize);
        let observed = plan(&case.before, &request, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(observed.after_bytes(), case.after);
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            let error = plan(&case.before, &request, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .expect("cancelled planner cannot publish a candidate");
            assert_eq!(inspection(error), InspectError::Cancelled);
        }
        assert_eq!(case.before, original);
    }
}

#[test]
#[ignore = "requires all four hash-pinned EOF source images"]
fn eof_claim_requires_global_free_even_when_original_scalar_source_is_healthy() {
    for case in cases() {
        let mut unavailable = case.before.clone();
        let global = 4096 + 4027 + 5 + case.target as usize / 8;
        let bit = 1 << (case.target % 8);
        assert_ne!(unavailable[global] & bit, 0);
        unavailable[global] &= !bit;
        readable(&unavailable);
        image(&unavailable)
            .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        let saved = unavailable.clone();
        let error = plan(
            &unavailable,
            &updates(sources::GROWN),
            &mut budget(),
            &|| false,
        )
        .err()
        .expect("an already-claimed GLOBAL cell is not allocatable capacity");
        assert_eq!(error, EditError::Unsupported(WriteBlocker::RowLayout));
        assert_eq!(unavailable, saved);
    }
}

#[test]
#[ignore = "requires all four hash-pinned EOF source images"]
fn eof_claim_refuses_future_owned_hint_and_cross_role_aliases_without_a_candidate() {
    for case in cases() {
        let (definition, maps, bitmap) = if case.header == 5 {
            (107_usize, 108_usize, 111_usize)
        } else {
            (125, 126, 128)
        };
        for variant in 0..4 {
            let mut invalid = case.before.clone();
            match variant {
                0 => {
                    let at = bitmap * 4096 + 4 + case.target as usize / 8;
                    invalid[at] |= 1 << (case.target % 8);
                }
                1 => {
                    let at = maps * 4096 + 3958 + 5 + case.target as usize / 8;
                    invalid[at] |= 1 << (case.target % 8);
                }
                2 | 3 => {
                    // Alias ordinary candidate with ordinary owned or readonly
                    // Bytes LVAL-owned storage. Scalar decoding still works;
                    // mutation source/future-claim ownership must refuse.
                    let slot = if variant == 2 { 0 } else { 2 };
                    let reference = ((maps as u32) << 8) | slot;
                    invalid[definition * 4096 + 59..definition * 4096 + 63]
                        .copy_from_slice(&reference.to_le_bytes());
                }
                _ => unreachable!(),
            }
            if variant >= 2 {
                readable(&invalid);
            }
            let saved = invalid.clone();
            // An out-of-EOF SET or role alias may be refused by the original
            // Source census before allocation. Do not invent a healthy-source
            // or a particular diagnostic claim for those malformed controls.
            assert!(plan(&invalid, &updates(sources::GROWN), &mut budget(), &|| false).is_err());
            assert_eq!(invalid, saved);
        }
    }
}

#[test]
#[ignore = "requires all four hash-pinned EOF source images"]
fn eof_selection_preserves_fitting_noop_and_empty_requests_without_unused_pages() {
    for case in cases() {
        readable(&case.before);
        image(&case.before)
            .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        for mode in 0..3 {
            let mut expected = case.before.clone();
            let (request, changed) = match mode {
                0 => {
                    let text = "qualified sentinel";
                    let bytes = text
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>();
                    assert_eq!(bytes.len(), 36);
                    let start = case.root as usize * 4096 + 4030 + 6;
                    expected[start..start + 36].copy_from_slice(&bytes);
                    (updates(text), 1)
                }
                1 => (updates(sources::OLD), 0),
                2 => (Vec::new(), 0),
                _ => unreachable!(),
            };
            let original = vec![
                NativeFittedIntentGroup::new(
                    "Marker",
                    NativeFittedUpdates::Variable(
                        request
                            .iter()
                            .map(|update| {
                                SlotScalarUpdate::new(
                                    update.row(),
                                    update.expected().clone(),
                                    update.value().clone(),
                                )
                            })
                            .collect(),
                    ),
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            ];
            let automatic = image(&case.before)
                .plan_recoverable_ordinary_variable_updates(
                    "Sentinel",
                    original,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                automatic.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFitted
            );
            assert_eq!(automatic.after_bytes(), expected);
            assert_eq!(automatic.changed_rows(), changed);
            assert_eq!(
                automatic.before_bytes().len(),
                automatic.after_bytes().len()
            );
            assert!(plan(&case.before, &request, &mut budget(), &|| false).is_err());
        }
    }
}
