//! Full public request framing/capture/Source/stage/manual-proof measurements.
use super::{all, budget, scope};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    RecoverableEditError,
};
use tablecraft_testkit::support::ace::resources::assert_public_boundary;

#[test]
#[ignore = "requires held IgnoreNULL original/AFTER images"]
fn single_omission_public_boundary_keeps_exact_pools_and_all_cancellation_positions() {
    let cases = all();
    assert_eq!(cases.len(), 12);
    for case in &cases {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let literal = case
                    .updates()
                    .into_iter()
                    .map(|(row, before, after)| FixedScalarUpdate::new(row, before, after))
                    .collect();
                let group = NativeFixedIntentGroup::new("Left", literal, work, cancelled).map_err(
                    |error| match error {
                        NativeIntentError::Inspect(error) => error,
                        other => panic!("unexpected literal request framing: {other:?}"),
                    },
                )?;
                let image =
                    NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)?;
                image
                    .plan_recoverable_ordinary_fixed_updates("Single", vec![group], work, cancelled)
                    .map_err(|error| match error {
                        RecoverableEditError::Edit(EditError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Request(
                            EditError::Inspect(error),
                        )) => error,
                        other => panic!("unexpected qualified omission plan: {other:?}"),
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
