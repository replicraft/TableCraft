//! Full public capture/intent/Source/physical/fresh-proof resource boundary.
use super::super::{POOLS, planning_inspection};
use super::{budget, cases, requests, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeCompleteInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::genuine_compound_root_split::Case;

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeCompleteInsertPlan, RecoverableEditError> {
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_complete_row_inserts(
        "BinaryStorage",
        requests(),
        work,
        cancelled,
    )
}

#[test]
#[ignore = "requires four read-qualified compound root-split images before producers"]
fn complete_public_compound_root_split_requires_exact_pools_and_observes_entry_middle_terminal_cancel()
 {
    let cases = cases();
    let mut headers = 0;
    for case in cases {
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        let planned = plan(&case, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(planned.before_bytes(), case.before);
        assert_eq!(planned.after_bytes(), case.after);
        assert_eq!(planned.inserted_rows(), 1);
        let costs: [u64; 3] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let terminal = polls.get();
        assert!(terminal > 2);
        drop(planned);

        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        let planned = plan(&case, &mut exact, &|| false).unwrap();
        assert_eq!(planned.before_bytes(), case.before);
        assert_eq!(planned.after_bytes(), case.after);
        assert_eq!(planned.inserted_rows(), 1);
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
        drop(planned);
        for pool in 0..POOLS.len() {
            let mut allowance = costs;
            allowance[pool] -= 1;
            let error = plan(
                &case,
                &mut ResourceBudget::new(allowance[0], allowance[1], allowance[2]),
                &|| false,
            )
            .err()
            .expect("one-less full public budget must not publish a plan");
            assert!(
                matches!(planning_inspection(error), InspectError::Limit(error) if error.resource == POOLS[pool])
            );
        }
        for stop in [1, terminal.div_ceil(2), terminal] {
            let seen = Cell::new(0_usize);
            let error = plan(&case, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .expect("cancellation must precede recoverable-plan publication");
            assert_eq!(planning_inspection(error), InspectError::Cancelled);
            assert_eq!(seen.get(), stop);
        }
        headers += 1;
    }
    assert_eq!(headers, 2);
}
