use super::{ParentIdentity, PublicationError, PublicationReceipt};
use crate::{IoError, progress::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

// Paid before staging/visibility; these allowances cover bounded cleanup and
// terminal callbacks even when the caller's remaining work pool is exhausted.
const CLEANUP_STEPS: u64 = 8;
const TERMINAL_STEPS: u64 = 4;

pub(super) enum PublishFailure {
    Collision(IoError),
    Uncertain(IoError),
}

pub(super) struct CreateFailure {
    pub cause: IoError,
    pub temporary_may_remain: bool,
}

pub(super) trait Backend {
    type Stage;

    fn identity(&self) -> &ParentIdentity;

    fn terminal_work(&self) -> u64 {
        0
    }

    fn create(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Stage, CreateFailure>;

    fn write(
        &self,
        stage: &mut Self::Stage,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IoError>;

    fn sync_payload(&self, stage: &Self::Stage) -> Result<(), IoError>;

    fn verify(&self, stage: &Self::Stage) -> Result<(), IoError>;

    fn publish(&self, stage: &Self::Stage) -> Result<(), PublishFailure>;

    fn cleanup(&self, stage: &Self::Stage) -> Result<(), IoError>;

    fn sync_parent(&self) -> Result<(), IoError>;
}

fn abort<B: Backend>(backend: &B, stage: &B::Stage, cause: IoError) -> PublicationError {
    let cleanup = backend.cleanup(stage).err();
    PublicationError::NotPublished {
        cause,
        temporary_may_remain: cleanup.is_some(),
        cleanup,
    }
}

pub(super) fn publish<B: Backend>(
    backend: B,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PublicationReceipt, PublicationError> {
    check(budget, cancelled).map_err(PublicationError::before)?;
    budget
        .charge(Resource::Bytes, bytes.len() as u64)
        .map_err(IoError::from)
        .map_err(PublicationError::before)?;
    budget
        .charge(Resource::Steps, CLEANUP_STEPS)
        .map_err(IoError::from)
        .map_err(PublicationError::before)?;

    let mut stage =
        backend
            .create(budget, cancelled)
            .map_err(|failure| PublicationError::NotPublished {
                cause: failure.cause,
                cleanup: None,
                temporary_may_remain: failure.temporary_may_remain,
            })?;

    let preparation = (|| {
        backend.write(&mut stage, bytes, budget, cancelled)?;
        check(budget, cancelled)?;
        backend.sync_payload(&stage)?;
        check(budget, cancelled)?;
        // Verification observes both the private-directory and payload entries.
        budget.charge(Resource::Steps, 1)?;
        backend.verify(&stage)?;
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, TERMINAL_STEPS)?;
        budget.charge(Resource::Steps, backend.terminal_work())?;
        Ok::<(), IoError>(())
    })();
    if let Err(cause) = preparation {
        return Err(abort(&backend, &stage, cause));
    }

    // No shared budget/cancellation polling after this visibility boundary.
    match backend.publish(&stage) {
        Ok(()) => {}
        Err(PublishFailure::Collision(cause)) => return Err(abort(&backend, &stage, cause)),
        Err(PublishFailure::Uncertain(cause)) => {
            // Neither cleanup nor another namespace callback can safely infer
            // what the failed publication callback actually made visible.
            return Err(PublicationError::VisibilityUncertain { cause });
        }
    }

    let cleanup = backend.cleanup(&stage).err();
    let barrier = backend.sync_parent().err();
    if let Some(cause) = cleanup {
        return Err(PublicationError::PublishedCleanupFailure {
            cause,
            namespace_synced: barrier.is_none(),
            barrier,
        });
    }
    if let Some(cause) = barrier {
        return Err(PublicationError::PublishedUnconfirmed { cause });
    }

    Ok(PublicationReceipt {
        parent: backend.identity().clone(),
    })
}
