//! Full measured native framing/capture/plan/proof; no hidden control pools.
use super::{all, budget, scope};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    RecoverableEditError,
};
use tablecraft_testkit::support::ace::resources::assert_public_boundary;

#[test]
#[ignore = "requires independently qualified wider-sibling source files"]
fn full_public_wider_sibling_operations_obey_exact_pools_and_entry_middle_terminal_cancellation() {
    let cases = all();
    assert_eq!(cases.len(), 24);
    for case in cases {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let values = case
                    .updates(false)
                    .into_iter()
                    .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
                    .collect();
                let group = NativeFixedIntentGroup::new("KeyValue", values, work, cancelled)
                    .map_err(|error| match error {
                        NativeIntentError::Inspect(error) => error,
                        other => panic!("unexpected original framing refusal: {other:?}"),
                    })?;
                let image =
                    NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)?;
                image
                    .plan_recoverable_ordinary_fixed_updates(
                        case.table,
                        vec![group],
                        work,
                        cancelled,
                    )
                    .map_err(|error| match error {
                        RecoverableEditError::Edit(EditError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Inspect(error)) => error,
                        other => panic!("unexpected source-backed refusal: {other:?}"),
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
