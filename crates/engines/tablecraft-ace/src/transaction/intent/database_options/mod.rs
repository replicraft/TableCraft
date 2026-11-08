//! Original explicit OFF request, complete source binding and resizable recovery.
mod codec;
mod planning;
mod recovery;
#[cfg(test)]
mod tests;

use super::{NativeIntentError, binding};
use crate::{DatabaseOptionsError, InspectError, NativeDatabaseOptionsPlan, header::check};
use tablecraft_core::limits::{LimitError, ResourceBudget};
use thiserror::Error;

pub use planning::RecoverableNativeDatabaseOptionsPlan;
pub use recovery::{QualifiedDatabaseOptionsRecovery, qualify_native_database_options_recovery};

/// Refusal before an explicit option request gains persistence/recovery authority.
#[derive(Debug, Error)]
pub enum DatabaseOptionsIntentError {
    /// Native inspection, allocation, cumulative limits or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete BEFORE digest binding failed.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// Exact native source/option construction failed.
    #[error(transparent)]
    Options(#[from] DatabaseOptionsError),
    /// Framing, action, flags, reserved bytes or source length is malformed.
    #[error("invalid database-options intent record")]
    InvalidRecord,
    /// The recognized action has an unknown wire version.
    #[error("database-options intent version is unsupported")]
    UnsupportedVersion,
    /// The recognized version selects an unqualified native source profile.
    #[error("database-options intent profile is unsupported")]
    UnsupportedProfile,
    /// Supplied complete original differs from the captured length/digest.
    #[error("database-options before image does not match")]
    BeforeMismatch,
    /// Fresh native reconstruction differs from the complete candidate.
    #[error("database-options reconstructed image does not match")]
    ImageMismatch,
    /// A shared image-proof operation failed outside a direct mismatch.
    #[error(transparent)]
    Proof(#[from] crate::NativeRecoveryQualificationError),
}

impl From<LimitError> for DatabaseOptionsIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// TCDO0001/profile 1: explicitly disable both database Name AutoCorrect options.
/// Decoding binds no permissions, physical patches, target path or AFTER image.
pub struct NativeDatabaseOptionsIntent {
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeDatabaseOptionsIntent {
    /// Exact fixed request size, before a host allocates retained request storage.
    pub const MAX_ENCODED_BYTES: usize = codec::RECORD_BYTES;

    fn capture(
        plan: &NativeDatabaseOptionsPlan,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, DatabaseOptionsIntentError> {
        check(budget, cancelled)?;
        let before_length = plan.before_bytes().len() as u64;
        codec::validate_length(before_length)?;
        if u64::from(plan.property_page()) * crate::format::page::PAGE_BYTES as u64 != before_length
        {
            return Err(DatabaseOptionsIntentError::InvalidRecord);
        }
        let before_binding = binding::digest(plan.before_bytes(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            before_length,
            before_binding,
        })
    }

    /// Encode this original explicit OFF request without write authority.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, DatabaseOptionsIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode only the exact action/version/profile; no fallback is attempted.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, DatabaseOptionsIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Check every original byte through the retained complete source binding.
    /// A match does not establish native source roles, permissions or save intent.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), DatabaseOptionsIntentError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.before_length {
            return Err(DatabaseOptionsIntentError::BeforeMismatch);
        }
        let actual = binding::digest(bytes, budget, cancelled)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(DatabaseOptionsIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}
