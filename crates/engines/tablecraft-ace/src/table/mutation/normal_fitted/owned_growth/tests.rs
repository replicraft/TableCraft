//! Direct normal physical planning; literal SOURCE images remain the oracle.
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
use tablecraft_testkit::access::genuine_owned_growth::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(147);
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

fn update() -> [SlotScalarUpdate; 1] {
    [SlotScalarUpdate::new(
        0,
        Value::Text(sources::OLD.into()),
        Value::Text(sources::GROWN.into()),
    )]
}

#[test]
#[ignore = "requires all six hash-pinned genuine owned-growth SOURCE images"]
fn physical_owned_growth_pays_exact_pools_and_cancels_before_returning_a_plan() {
    for case in cases() {
        let updates = update();
        let groups = [FittedColumnUpdates::Variable {
            column: "Marker",
            updates: &updates,
        }];
        // Capture and borrowed requests are outside the measured physical call.
        let run = |work: &mut ResourceBudget, cancel: &dyn Fn() -> bool| {
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
        let counted = run(&mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(counted.after_bytes(), case.after);
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

#[test]
#[ignore = "requires all six hash-pinned genuine owned-growth SOURCE images"]
fn healthy_original_with_exhausted_slot_directory_refuses_growth_atomically() {
    for case in cases() {
        let mut before = case.before.clone();
        let base = case.data as usize * 4096;
        let page = &mut before[base..base + 4096];
        // Independent format witnesses: original live slot0 begins4030 and
        // retains its66B body. Empty deleted slots retain that same frontier.
        assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(page[14..16].try_into().unwrap()), 4030);
        page[12..14].copy_from_slice(&255_u16.to_le_bytes());
        for slot in 1..255 {
            let at = 14 + slot * 2;
            page[at..at + 2].copy_from_slice(&(0x8000_u16 | 4030).to_le_bytes());
        }
        page[2..4].copy_from_slice(&(4030_u16 - (14 + 255 * 2)).to_le_bytes());
        let original = image(&before);
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
                Value::Text(sources::OLD.into()),
                Value::Binary(vec![199, 0, 255]),
            ]]
        );
        // Full native target-local closure, LVAL ownership and row capture must
        // accept this original before exercising any physical mutation code.
        let source = super::capture(&original, "Sentinel", &mut budget(), &|| false).unwrap();
        assert_eq!(source.table.rows(), table.rows());
        assert_eq!(original.as_bytes(), before);
        let updates = update();
        let groups = [FittedColumnUpdates::Variable {
            column: "Marker",
            updates: &updates,
        }];
        assert!(matches!(
            original.plan_ordinary_owned_growth_updates(
                "Sentinel",
                &groups,
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
        // No candidate is returned and the caller's entire original is intact.
        assert_eq!(
            &before[base + 4030..base + 4096],
            &case.before[base + 4030..base + 4096]
        );
    }
}

#[test]
#[ignore = "requires all six hash-pinned genuine owned-growth SOURCE images"]
fn final_available_slot_after_empty_deleted_entries_preserves_every_original_slot() {
    for case in cases() {
        let base = case.data as usize * 4096;
        let mut before = case.before.clone();
        before[base + 12..base + 14].copy_from_slice(&254_u16.to_le_bytes());
        before[base + 2..base + 4].copy_from_slice(&3508_u16.to_le_bytes());
        for slot in 1..254 {
            let at = base + 14 + slot * 2;
            before[at..at + 2].copy_from_slice(&0x8fbe_u16.to_le_bytes());
        }
        let original = image(&before);
        let source = super::capture(&original, "Sentinel", &mut budget(), &|| false).unwrap();
        assert_eq!(source.table.rows().len(), 1);

        // The terminal comes from the separately constructed, pinned source
        // image. Only this derivative's literal directory/root identity differs.
        let mut expected = before.clone();
        expected[base + 3962..base + 4030].copy_from_slice(&case.after[base + 3962..base + 4030]);
        expected[base + 4030..base + 4096].fill(0);
        expected[base + 4030..base + 4034].copy_from_slice(&((case.data << 8) | 254).to_le_bytes());
        expected[base + 14..base + 16].copy_from_slice(&0x4fbe_u16.to_le_bytes());
        expected[base + 522..base + 524].copy_from_slice(&0x8f7a_u16.to_le_bytes());
        expected[base + 12..base + 14].copy_from_slice(&255_u16.to_le_bytes());
        expected[base + 2..base + 4].copy_from_slice(&3438_u16.to_le_bytes());

        let updates = update();
        let groups = [FittedColumnUpdates::Variable {
            column: "Marker",
            updates: &updates,
        }];
        let plan = original
            .plan_ordinary_owned_growth_updates("Sentinel", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);
        let changed = image(plan.after_bytes());
        let table = read_table(
            &mut changed.reader(),
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
                Value::Text(sources::GROWN.into()),
                Value::Binary(vec![199, 0, 255]),
            ]]
        );
    }
}
