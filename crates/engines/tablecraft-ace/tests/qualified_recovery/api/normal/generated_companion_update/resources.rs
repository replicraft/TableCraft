//! Measured full capture/request binding/Source/stage/proof public boundary.
use super::{all, budget, groups, scope};
use std::io::Cursor;
use tablecraft_ace::{EditError, NativeImage, NativeIntentError, RecoverableEditError};
use tablecraft_testkit::support::ace::resources::assert_public_boundary;

#[test]
#[ignore = "requires twelve independently qualified generated-companion Score images"]
fn generated_companion_full_public_plans_obey_all_exact_pools_and_entry_middle_terminal_cancellation()
 {
    let cases = all();
    assert_eq!(cases.len(), 16);
    for case in &cases {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let requests = groups(case, false);
                let image =
                    NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)?;
                image
                    .plan_recoverable_ordinary_fixed_updates(case.table, requests, work, cancelled)
                    .map_err(|error| match error {
                        RecoverableEditError::Edit(EditError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Inspect(error)) => error,
                        other => panic!("unexpected source-backed public refusal: {other:?}"),
                    })
            },
            |plan| {
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.changed_rows(), case.changed());
            },
            |plan| {
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.changed_rows(), case.changed());
            },
        );
    }
}
