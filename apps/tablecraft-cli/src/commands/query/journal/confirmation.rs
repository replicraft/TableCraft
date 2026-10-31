//! Confirm an intact retired bundle without replaying its native edit.
use super::{Error, FinalizeArguments, report};
use crate::native;
use std::{fmt, io::Write};
use tablecraft_ace::{InspectError, NativeInt32EditIntent, qualify_jet4_int32_recovery};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{
    RecoveryOutcome,
    filesystem::{OfflineJournalPair, OfflineTarget, PairSetupFailure},
    inspect_v1,
};

const NAMESPACE_REPORT: &str = "Journal namespace\tRetired and synchronized\n";
const RETAINED_OPEN_FAILURES: usize = 1;

/// A verified current complete image, not a claim about historical operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VerifiedImage {
    Original,
    Replacement,
}

impl VerifiedImage {
    fn from_confirmation(outcome: RecoveryOutcome) -> Self {
        match outcome {
            RecoveryOutcome::Committed => Self::Replacement,
            RecoveryOutcome::RolledBack => Self::Original,
        }
    }
}

impl fmt::Display for VerifiedImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Original => "Original",
            Self::Replacement => "Replacement",
        })
    }
}

struct Reports {
    original: String,
    replacement: String,
}

impl Reports {
    fn prepare(budget: &mut ResourceBudget) -> Result<Self, Error> {
        let mut original = report::buffer(budget).map_err(Error::ConfirmationResources)?;
        original.push_str("Verified image\tOriginal\n");
        original.push_str(NAMESPACE_REPORT);

        let mut replacement = report::buffer(budget).map_err(Error::ConfirmationResources)?;
        replacement.push_str("Verified image\tReplacement\n");
        replacement.push_str(NAMESPACE_REPORT);

        Ok(Self {
            original,
            replacement,
        })
    }

    fn publish(self, image: VerifiedImage, writer: &mut dyn Write) -> Result<(), Error> {
        let text = match image {
            VerifiedImage::Original => self.original,
            VerifiedImage::Replacement => self.replacement,
        };
        writer
            .write_all(text.as_bytes())
            .and_then(|()| writer.flush())
            .map_err(|error| Error::VerifiedOutput { image, error })
    }
}

pub(super) fn run(args: FinalizeArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.finalize_update").entered();
    let mut budget = args.work.budget();
    let mut retained = reserve_open_failure(&mut budget)?;
    let target = OfflineTarget::open(&args.database, &mut budget, &|| false).map_err(Error::Io)?;
    let mut pair = OfflineJournalPair::open_retired(
        target,
        &args.journal,
        NativeInt32EditIntent::MAX_ENCODED_BYTES as u64,
        &mut budget,
        &|| false,
    )
    .map_err(|error| {
        // Capacity was paid and reserved before opening the target. Keep the
        // complete typed cause and retained capabilities without late allocation.
        retained.push(error);
        Error::RetiredOpen(retained)
    })?;
    let intent = NativeInt32EditIntent::decode(pair.intent_bytes(), &mut budget, &|| false)
        .map_err(Error::ConfirmationIntent)?;
    let reports = Reports::prepare(&mut budget)?;

    let outcome = {
        // These views deny payload writes/flush before reaching the underlying
        // devices. Confirmation checks the current image and performs barriers;
        // it never invokes the active-journal recovery operation.
        let (mut target, mut journal) = pair.confirmation_storage().map_err(Error::RetiredPair)?;
        let checked =
            inspect_v1(&mut journal, &mut budget, &|| false).map_err(Error::Confirmation)?;
        let qualified = qualify_jet4_int32_recovery(
            checked,
            intent,
            native::CATALOG_SCOPE,
            &mut budget,
            &|| false,
        )
        .map_err(Error::ConfirmationQualification)?;
        qualified
            .confirm_completed(&mut target, &mut budget, &|| false)
            .map_err(Error::Confirmation)?
    };
    let image = VerifiedImage::from_confirmation(outcome);

    // The image/barrier fact is retained separately from namespace completion.
    // No rename, recovery retry or SQL execution is authorized by this branch.
    pair.acknowledge_retired(outcome)
        .map_err(|error| Error::VerifiedFinalization { image, error })?;
    pair.cleanup_retired()
        .map_err(|error| Error::VerifiedFinalization { image, error })?;
    pair.release()
        .map_err(|error| Error::VerifiedRelease { image, error })?;
    reports.publish(image, writer)
}

fn reserve_open_failure(budget: &mut ResourceBudget) -> Result<Vec<PairSetupFailure>, Error> {
    budget
        .charge_allocation(
            RETAINED_OPEN_FAILURES as u64,
            size_of::<PairSetupFailure>() as u64,
        )
        .map_err(InspectError::from)
        .map_err(Error::ConfirmationResources)?;
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)
        .map_err(Error::ConfirmationResources)?;
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(RETAINED_OPEN_FAILURES)
        .map_err(|_| Error::ConfirmationResources(InspectError::Allocation))?;
    Ok(retained)
}

#[cfg(test)]
mod tests;
