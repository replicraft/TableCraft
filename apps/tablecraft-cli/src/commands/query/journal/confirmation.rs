//! Confirm an intact retired bundle without replaying its native edit.
use super::{Error, FinalizeArguments, error::UnexpectedReceipt, report};
use crate::native;
use std::{fmt, io::Write};
use tablecraft_access::editing::offline::{
    OfflineWriteSession, VerifiedImage as NativeVerifiedImage,
};
use tablecraft_core::limits::ResourceBudget;

/// A verified current complete image, not a claim about historical operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VerifiedImage {
    Original,
    Replacement,
}

impl VerifiedImage {
    fn from_confirmation(image: NativeVerifiedImage) -> Self {
        match image {
            NativeVerifiedImage::Replacement => Self::Replacement,
            NativeVerifiedImage::Original => Self::Original,
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

        let mut replacement = report::buffer(budget).map_err(Error::ConfirmationResources)?;
        replacement.push_str("Verified image\tReplacement\n");

        Ok(Self {
            original,
            replacement,
        })
    }

    fn publish(
        self,
        image: VerifiedImage,
        namespace: report::NamespaceFacts,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        let text = match image {
            VerifiedImage::Original => self.original,
            VerifiedImage::Replacement => self.replacement,
        };
        writer
            .write_all(text.as_bytes())
            .and_then(|()| namespace.publish(writer))
            .and_then(|()| writer.flush())
            .map_err(|error| Error::VerifiedOutput { image, error })
    }
}

pub(super) fn run(args: FinalizeArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.finalize_update").entered();
    let mut budget = args.work.budget();
    let session =
        OfflineWriteSession::open(&args.database, native::CATALOG_SCOPE, &mut budget, &|| {
            false
        })
        .map_err(Error::OfflineOpen)?;
    let reports = Reports::prepare(&mut budget)?;
    let (session, receipt) = session
        .confirm_retired(&args.journal, &mut budget, &|| false)
        .map_err(Error::Offline)?;
    let Some(verified) = receipt.verified_image() else {
        return Err(Error::UnexpectedReceipt(UnexpectedReceipt {
            session,
            receipt,
        }));
    };
    let image = VerifiedImage::from_confirmation(verified);

    // The shared receipt certifies the current image, without asserting that a
    // historical commit or rollback occurred. Report only after explicit release.
    session.release().map_err(Error::Offline)?;
    reports.publish(image, report::NamespaceFacts::from_receipt(receipt), writer)
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
