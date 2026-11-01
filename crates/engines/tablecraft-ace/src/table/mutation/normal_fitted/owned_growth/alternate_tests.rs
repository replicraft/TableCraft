//! Independent alternate-owned SOURCE and direct physical planner controls.
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, SlotScalarUpdate, WriteBlocker,
    read_table,
};
use sha2::{Digest, Sha256};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    genuine_alternate_owned::{self as sources, Case},
    genuine_owned_growth as root_sources,
};

const SCOPE: CatalogScope = CatalogScope::new(149);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn updates() -> [SlotScalarUpdate; 1] {
    [SlotScalarUpdate::new(
        0,
        Value::Text("unchanged sentinel".into()),
        Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
    )]
}

#[test]
#[ignore = "requires the pinned genuine alternate-owned SOURCE artifacts"]
fn alternate_owned_physical_limits_and_cancellation_return_no_partial_plan() {
    for case in cases() {
        let updates = updates();
        let groups = [FittedColumnUpdates::Variable {
            column: "Marker",
            updates: &updates,
        }];
        let run = |work: &mut ResourceBudget, cancel: &dyn Fn() -> bool| {
            // Independent capture does not consume the measured planner pools.
            image(&case.before)
                .plan_ordinary_owned_growth_updates("Sentinel", &groups, work, cancel)
        };
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let plan = run(&mut measured, &|| false).unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), 1);
        let costs: [u64; 3] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        let exact = run(
            &mut ResourceBudget::new(costs[0], costs[1], costs[2]),
            &|| false,
        )
        .unwrap();
        assert_eq!(exact.after_bytes(), case.after);
        for (index, pool) in POOLS.into_iter().enumerate() {
            assert!(costs[index] > 0);
            let mut below = costs;
            below[index] -= 1;
            assert!(matches!(
                run(&mut ResourceBudget::new(below[0], below[1], below[2]), &|| false),
                Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == pool
            ));
        }
        let polls = Cell::new(0_usize);
        run(&mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            assert!(matches!(
                run(&mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    seen.get() >= stop
                }),
                Err(EditError::Inspect(InspectError::Cancelled))
            ));
        }
    }
}

fn require_rows_and_source(bytes: &[u8], marker: &str) {
    let original = image(bytes);
    let table = read_table(
        &mut original.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        &[vec![
            Value::Int32(23),
            Value::Text(marker.into()),
            Value::Binary(vec![199, 0, 255]),
        ]]
    );
    let source = super::capture(&original, "Sentinel", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), table.rows());
}

#[test]
#[ignore = "requires the pinned genuine alternate-owned SOURCE artifacts"]
fn alternate_owned_capacity_and_hint_exhaustion_refuse_atomically() {
    for case in cases() {
        for control in 0..3 {
            let mut before = case.before.clone();
            let target = case.target as usize * 4096;
            let maps = if case.header == 5 { 108 } else { 126 };
            match control {
                0 => {
                    // No candidate hint; physical free capacity is not authority.
                    let at = maps * 4096 + 3958 + 5 + case.target as usize / 8;
                    before[at] &= !(1 << (case.target % 8));
                }
                1 => {
                    // All255 slots are deleted and empty; bytes remain available.
                    before[target + 12..target + 14].copy_from_slice(&255_u16.to_le_bytes());
                    before[target + 2..target + 4].copy_from_slice(&3572_u16.to_le_bytes());
                    for slot in 0..255 {
                        let at = target + 14 + 2 * slot;
                        before[at..at + 2].copy_from_slice(&0x9000_u16.to_le_bytes());
                    }
                }
                2 => {
                    // One bounded deleted extent consumes every append byte.
                    before[target + 12..target + 14].copy_from_slice(&1_u16.to_le_bytes());
                    before[target + 2..target + 4].copy_from_slice(&0_u16.to_le_bytes());
                    before[target + 14..target + 16].copy_from_slice(&0x8010_u16.to_le_bytes());
                }
                _ => unreachable!(),
            }
            require_rows_and_source(&before, "unchanged sentinel");
            let updates = updates();
            let groups = [FittedColumnUpdates::Variable {
                column: "Marker",
                updates: &updates,
            }];
            assert!(matches!(
                image(&before).plan_ordinary_owned_growth_updates(
                    "Sentinel",
                    &groups,
                    &mut budget(),
                    &|| false,
                ),
                Err(EditError::Unsupported(WriteBlocker::RowLayout))
            ));
        }
    }
}

#[test]
#[ignore = "requires the pinned genuine alternate-owned SOURCE artifacts"]
fn alternate_owned_unknown_or_shared_roles_never_return_a_candidate() {
    for case in cases() {
        for control in 0..4 {
            let mut before = case.before.clone();
            let (definition, maps, bitmap) = if case.header == 5 {
                (107_usize, 108_usize, 111_usize)
            } else {
                (125_usize, 126_usize, 128_usize)
            };
            match control {
                0 => {
                    let at = 4096 + 4027 + 5 + case.target as usize / 8;
                    before[at] |= 1 << (case.target % 8);
                }
                1 => {
                    let at = case.target as usize * 4096 + 4;
                    before[at..at + 4].copy_from_slice(&2_u32.to_le_bytes());
                }
                2 => {
                    // Actual declared OLE owned-map cell aliases table DATA map.
                    let at = definition * 4096 + 172;
                    before[at..at + 4].copy_from_slice(&((maps as u32) << 8).to_le_bytes());
                }
                3 => {
                    // Selected DATA membership overlaps actual map-storage page.
                    let at = bitmap * 4096 + 4 + maps / 8;
                    before[at] |= 1 << (maps % 8);
                }
                _ => unreachable!(),
            }
            let updates = updates();
            let groups = [FittedColumnUpdates::Variable {
                column: "Marker",
                updates: &updates,
            }];
            assert!(
                image(&before)
                    .plan_ordinary_owned_growth_updates("Sentinel", &groups, &mut budget(), &|| {
                        false
                    },)
                    .is_err()
            );
        }
    }
}

#[test]
#[ignore = "requires pinned alternate and original same-root growth SOURCE artifacts"]
fn available_original_root_wins_even_when_another_owned_candidate_is_empty() {
    let roots = root_sources::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    for case in cases() {
        let root = roots
            .iter()
            .find(|root| root.header == case.header)
            .unwrap();
        let base = case.root as usize * 4096;
        let mut before = case.before.clone();
        // Independently qualified original root, plus the new already-owned
        // empty page. Both actual hint bits are present before planning.
        before[base..base + 4096].copy_from_slice(&root.before[base..base + 4096]);
        let maps = if case.header == 5 { 108 } else { 126 };
        let at = maps * 4096 + 3958 + 5 + case.root as usize / 8;
        before[at] |= 1 << (case.root % 8);
        let mut expected = before.clone();
        expected[base..base + 4096].copy_from_slice(&root.after[base..base + 4096]);
        // Both derivative views are native/read-Source qualified before writer.
        require_rows_and_source(&before, "unchanged sentinel");
        require_rows_and_source(&expected, "aaaaaaaaaaaaaaaaaaa");
        let updates = updates();
        let groups = [FittedColumnUpdates::Variable {
            column: "Marker",
            updates: &updates,
        }];
        let plan = image(&before)
            .plan_ordinary_owned_growth_updates("Sentinel", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);
        let target = case.target as usize * 4096;
        assert_eq!(
            &plan.after_bytes()[target..target + 4096],
            &before[target..target + 4096]
        );
    }
}
