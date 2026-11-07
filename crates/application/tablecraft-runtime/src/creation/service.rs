//! The worker's fresh creation operation uses one cumulative native allowance.
use super::{CreationFailure, CreationOutcome, CreationPhase, CreationReceipt};
use crate::controls::budget;
use std::path::Path;
use tablecraft_access::creation::CreationDestination;

pub(super) fn blank(destination: &Path, cancelled: &dyn Fn() -> bool) -> CreationOutcome {
    let _span = tracing::debug_span!("application.create_blank_ace16").entered();
    let mut work = budget();
    let destination = match CreationDestination::capture(destination, &mut work, cancelled) {
        Ok(destination) => destination,
        Err(error) => {
            return CreationOutcome::Failed(CreationFailure::native(CreationPhase::Capture, error));
        }
    };

    let prepared = match destination.prepare_blank(&mut work, cancelled) {
        Ok(prepared) => prepared,
        Err(error) => {
            return CreationOutcome::Failed(CreationFailure::native(
                CreationPhase::Construct,
                error,
            ));
        }
    };

    match prepared.publish(&mut work, cancelled) {
        Ok(receipt) => CreationOutcome::Created(CreationReceipt::new(receipt)),
        Err(error) => {
            CreationOutcome::Failed(CreationFailure::native(CreationPhase::Publish, error))
        }
    }
}
