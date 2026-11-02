//! Public original request planning uses measured exact pools and cancellation.
use super::super::model as journals;
use super::{Case, NativeUpdateIntent, Recipe, Storage, all, budget, groups, scope};
use journals::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, NativeIntentError, RecoverableEditError,
};
use tablecraft_ace::{NativeRecoveryQualificationError, qualify_native_update_recovery};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::inspect_v1;

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn execute(
    case: &Case,
    recovery: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    if recovery {
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let saved = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent =
            NativeUpdateIntent::decode(&super::original_wire(case), &mut budget(), &|| false)
                .unwrap();
        let result = qualify_native_update_recovery(checked, intent, scope(), work, cancelled)
            .map(drop)
            .map_err(|error| match error {
                NativeRecoveryQualificationError::Inspect(error)
                | NativeRecoveryQualificationError::Edit(EditError::Inspect(error))
                | NativeRecoveryQualificationError::Intent(NativeIntentError::Inspect(error)) => {
                    error
                }
                other => panic!("unexpected original recovery refusal: {other:?}"),
            });
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);
        return result;
    }
    // Caller-owned original requests precede the measured native operation.
    let requests = groups(case, false);
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)?;
    let plan = image
        .plan_recoverable_ordinary_fixed_updates(case.table, requests, work, cancelled)
        .map_err(|error| match error {
            RecoverableEditError::Edit(EditError::Inspect(error))
            | RecoverableEditError::Intent(NativeIntentError::Inspect(error)) => error,
            other => panic!("unexpected qualified operation failure: {other:?}"),
        })?;
    assert_eq!(plan.after_bytes(), case.after);
    Ok(())
}

#[test]
#[ignore = "requires independently qualified wider UPDATE source images"]
fn retained_node_updates_obey_all_measured_public_pools_and_entry_middle_terminal_cancellation() {
    for case in all()
        .iter()
        .filter(|case| matches!(case.recipe, Recipe::Joint | Recipe::Shift))
    {
        for recovery in [false, true] {
            let mut measured = budget();
            let initial = POOLS.map(|pool| measured.remaining(pool));
            let polls = Cell::new(0_usize);
            execute(case, recovery, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            let used = std::array::from_fn::<_, 3, _>(|position| {
                initial[position] - measured.remaining(POOLS[position])
            });
            assert!(used.iter().all(|amount| *amount > 0));
            let mut exact = ResourceBudget::new(used[0], used[1], used[2]);
            execute(case, recovery, &mut exact, &|| false).unwrap();
            for pool in POOLS {
                assert_eq!(exact.remaining(pool), 0);
            }
            for position in 0..3 {
                let mut limits = used;
                limits[position] -= 1;
                let mut short = ResourceBudget::new(limits[0], limits[1], limits[2]);
                assert!(
                    matches!(execute(case, recovery, &mut short, &|| false), Err(InspectError::Limit(error)) if error.resource == POOLS[position])
                );
            }
            for stop in [1, polls.get().div_ceil(2), polls.get()] {
                let seen = Cell::new(0_usize);
                assert_eq!(
                    execute(case, recovery, &mut budget(), &|| {
                        seen.set(seen.get() + 1);
                        seen.get() == stop
                    }),
                    Err(InspectError::Cancelled)
                );
                assert_eq!(seen.get(), stop);
            }
        }
    }
}
