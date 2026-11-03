//! Measured full capture/request binding/Source/stage/proof public boundary.
use super::{all, budget, scope};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    RecoverableEditError,
};
use tablecraft_testkit::support::ace::resources::assert_public_boundary;

#[test]
#[ignore = "requires four independently qualified Binary Id images"]
fn binary_id_full_public_plans_obey_all_exact_pools_and_entry_middle_terminal_cancellation() {
    let cases = all();
    assert_eq!(cases.len(), 6);
    for case in &cases {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let updates = case
                    .updates()
                    .into_iter()
                    .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
                    .collect();
                let group = NativeFixedIntentGroup::new("Id", updates, work, cancelled).map_err(
                    |error| match error {
                        NativeIntentError::Inspect(error) => error,
                        other => panic!("unexpected original request framing refusal: {other:?}"),
                    },
                )?;
                let requests = vec![group];
                let image =
                    NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)?;
                image
                    .plan_recoverable_ordinary_fixed_updates(
                        "BinaryKeys",
                        requests,
                        work,
                        cancelled,
                    )
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
