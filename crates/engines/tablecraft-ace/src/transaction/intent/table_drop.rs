//! Original table DROP requests bind the complete source, never replacement authority.
mod codec;
#[cfg(test)]
mod current_tests;
mod planning;
mod recovery;
#[cfg(test)]
mod tests;

use super::{NativeIntentError, binding};
use crate::{
    InspectError, NativeImage, NativeRecoveryQualificationError, NativeTableDropIdentity,
    NativeTableDropPlan, TableDropError, header::check, table::mutation::drop_table::SourceProfile,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

pub use planning::RecoverableTableDropPlan;
pub use recovery::{
    QualifiedRetainedTableDropRecovery, QualifiedTableDropRecovery,
    qualify_native_table_drop_recovery,
};

const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Payload-free request decoding, native reconstruction or full-image failure.
#[derive(Debug, Error)]
pub enum TableDropIntentError {
    /// Native parsing, cancellation, allocation or resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete original-image binding failure.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// Native source or table dependencies are not qualified.
    #[error(transparent)]
    Drop(#[from] TableDropError),
    /// Full original/replacement comparison failure.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// Closed action/version/profile, framing or name is invalid.
    #[error("invalid native table DROP intent record")]
    InvalidRecord,
    /// Complete original image differs from the reviewed source.
    #[error("native table DROP original image does not match")]
    BeforeMismatch,
    /// Actual native table identity or spelling differs.
    #[error("native table DROP request identity does not match")]
    IdentityMismatch,
}

impl From<LimitError> for TableDropIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    native_id: i32,
    parent_id: i32,
    definition: u32,
    catalog_page: u32,
    catalog_slot: u8,
}

impl Identity {
    fn from_native(identity: NativeTableDropIdentity) -> Self {
        Self {
            native_id: identity.native_id(),
            parent_id: identity.parent_id(),
            definition: identity.definition_page(),
            catalog_page: identity.catalog_page(),
            catalog_slot: identity.catalog_slot(),
        }
    }
}

/// Data-only original action and complete BEFORE binding. Decoding confers no
/// native source, journal, AFTER or recovery permission.
pub struct NativeTableDropIntent {
    profile: SourceProfile,
    identity: Identity,
    table: String,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl NativeTableDropIntent {
    /// Closed bound checked before persisted request text is allocated.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    fn capture(
        plan: &NativeTableDropPlan,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableDropIntentError> {
        validate_name(plan.table(), budget, cancelled)?;
        let identity = Identity::from_native(plan.identity());
        let before_length = plan.before_bytes().len() as u64;
        let profile = plan.source_profile();
        codec::validate_identity(identity, before_length, profile)?;
        let before_binding = digest_before(profile, plan.before_bytes(), budget, cancelled)?;
        let table = copy_name(plan.table(), budget, cancelled)?;

        Ok(Self {
            profile,
            identity,
            table,
            before_length,
            before_binding,
        })
    }

    /// Exact original native table spelling, never a caption.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Encode the original action and BEFORE digest, without physical edit spans.
    ///
    /// # Errors
    /// Returns an error for invalid request data, cancellation or exhausted resources.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, TableDropIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode only TCTD0001 and its frozen source profiles, without fallback.
    ///
    /// # Errors
    /// Returns an error for other families/versions, malformed framing or resource failure.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableDropIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Decode only TCTD0002 current-source requests, without retrying older families.
    ///
    /// # Errors
    /// Returns an error for other families, malformed framing or resource failure.
    pub fn decode_current(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableDropIntentError> {
        codec::decode_current(bytes, budget, cancelled)
    }

    /// Decode only TCTD0003 retained requests; older source families stay closed.
    ///
    /// # Errors
    /// Returns an error for another family, malformed data or exhausted resources.
    pub fn decode_retained(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableDropIntentError> {
        codec::decode_retained(bytes, budget, cancelled)
    }

    /// Whether this data-only request belongs to the retained V3 source family.
    pub fn is_retained(&self) -> bool {
        self.profile == SourceProfile::Retained
    }

    /// Verify every original byte before reconstructing native authority.
    ///
    /// # Errors
    /// Returns an error for a different complete source, cancellation or exhausted resources.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), TableDropIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length
            || digest_before(self.profile, before, budget, cancelled)? != self.before_binding
        {
            return Err(TableDropIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableDropPlan, TableDropIntentError> {
        let plan =
            image.plan_table_drop_for_profile(self.table(), self.profile, budget, cancelled)?;
        if Identity::from_native(plan.identity()) != self.identity || plan.table() != self.table() {
            return Err(TableDropIntentError::IdentityMismatch);
        }
        Ok(plan)
    }
}

fn digest_before(
    profile: SourceProfile,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; binding::DIGEST_BYTES], TableDropIntentError> {
    let digest = if profile == SourceProfile::Retained {
        binding::digest_retained(before, budget, cancelled)?
    } else {
        binding::digest(before, budget, cancelled)?
    };
    Ok(digest)
}

fn validate_name(
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableDropIntentError> {
    check(budget, cancelled)?;
    if table.len() > MAX_NAME_BYTES || !crate::identifier::valid_name(table, budget, cancelled)? {
        return Err(TableDropIntentError::InvalidRecord);
    }
    Ok(())
}

fn copy_name(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, TableDropIntentError> {
    check(budget, cancelled)?;
    budget.charge_allocation(name.len() as u64, size_of::<u8>() as u64)?;
    budget.charge(Resource::Steps, name.len() as u64)?;
    let mut copy = String::new();
    copy.try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    copy.push_str(name);
    check(budget, cancelled)?;
    Ok(copy)
}
