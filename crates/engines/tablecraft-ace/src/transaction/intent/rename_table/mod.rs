//! Original catalog rename requests bound to one complete source image.
mod codec;
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
use crate::{InspectError, NativeTableRenamePlan, header::check};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub use planning::{
    PreparedTableRename, RecoverableNativeTableRenamePlan, RecoverableTableRenameError,
};
pub use recovery::{QualifiedTableRenameRecovery, qualify_native_table_rename_recovery};

const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Malformed request data or source binding, never native mutation authority.
#[derive(Debug, Error)]
pub enum TableRenameIntentError {
    /// Bounded inspection, allocation, cancellation or resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The shared full-source binding service refused the original bytes.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The closed framing, native identity or name invariants are malformed.
    #[error("invalid table-rename intent record")]
    InvalidRecord,
    /// The action prefix is recognized but its wire version is unsupported.
    #[error("table-rename intent version is unsupported")]
    UnsupportedVersion,
    /// The record selects an unsupported native rename profile.
    #[error("table-rename intent profile is unsupported")]
    UnsupportedProfile,
    /// The complete source length or digest differs from the retained original.
    #[error("table-rename intent before image does not match")]
    BeforeMismatch,
    /// Native identity, spellings or actual admission purpose differs.
    #[error("table-rename intent native identity does not match")]
    IdentityMismatch,
}

impl From<LimitError> for TableRenameIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Persistent native identity only; model ObjectId and its scope are not encoded.
struct Identity {
    native_id: i32,
    parent_id: i32,
    definition_page: u32,
    catalog_page: u32,
    catalog_slot: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    UnindexedV1,
    RetainedInt32V2,
}

impl Profile {
    fn from_plan(plan: &NativeTableRenamePlan) -> Self {
        if plan.has_indexed_admission() {
            Self::RetainedInt32V2
        } else {
            Self::UnindexedV1
        }
    }
}

/// Exact original and proposed spellings, native identity and complete BEFORE binding.
/// Decoding this data does not qualify the source, dependencies or replacement image.
pub struct NativeTableRenameIntent {
    profile: Profile,
    table: SchemaName,
    new_name: SchemaName,
    identity: Identity,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeTableRenameIntent {
    /// Bounded before the caller allocates a persisted intent record.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    fn capture(
        plan: &NativeTableRenamePlan,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableRenameIntentError> {
        check(budget, cancelled)?;
        let identity = plan.identity();
        let table = copy_name(plan.table(), budget, cancelled)?;
        let new_name = copy_name(plan.new_name(), budget, cancelled)?;
        let before_binding = binding::digest(plan.before_bytes(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            profile: Profile::from_plan(plan),
            table,
            new_name,
            identity: Identity {
                native_id: identity.native_id(),
                parent_id: identity.parent_id(),
                definition_page: identity.definition_page(),
                catalog_page: identity.catalog_page(),
                catalog_slot: identity.catalog_slot(),
            },
            before_length: plan.before_bytes().len() as u64,
            before_binding,
        })
    }

    /// Exact original catalog spelling retained by this data-only request.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact proposed spelling, without implicit normalization or dependency repair.
    pub fn new_name(&self) -> &str {
        self.new_name.as_str()
    }

    /// Encode the exact version/profile selected by the native planner's purpose.
    /// Encoded bytes contain intent and source binding, never native save authority.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, TableRenameIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Validate closed framing and name invariants before copying decoded names.
    /// The returned request still needs fresh source and native qualification.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableRenameIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Check the complete supplied source against the retained length and digest.
    /// Matching bytes alone do not qualify namespace, dependencies or journaling.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), TableRenameIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(TableRenameIntentError::BeforeMismatch);
        }
        if binding::digest(before, budget, cancelled)? != self.before_binding {
            return Err(TableRenameIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn verify_plan(&self, plan: &NativeTableRenamePlan) -> Result<(), TableRenameIntentError> {
        let actual = plan.identity();
        if Profile::from_plan(plan) != self.profile
            || actual.native_id() != self.identity.native_id
            || actual.parent_id() != self.identity.parent_id
            || actual.definition_page() != self.identity.definition_page
            || actual.catalog_page() != self.identity.catalog_page
            || actual.catalog_slot() != self.identity.catalog_slot
            || plan.table() != self.table()
            || plan.new_name() != self.new_name()
        {
            return Err(TableRenameIntentError::IdentityMismatch);
        }
        Ok(())
    }

    fn reconstruct(
        &self,
        image: crate::NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableRenamePlan, crate::TableRenameError> {
        match self.profile {
            Profile::UnindexedV1 => {
                image.plan_table_rename(self.table(), self.new_name(), budget, cancelled)
            }
            Profile::RetainedInt32V2 => {
                image.plan_indexed_table_rename(self.table(), self.new_name(), budget, cancelled)
            }
        }
    }
}

fn validate_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameIntentError> {
    check(budget, cancelled)?;
    if text.is_empty() || text.len() > MAX_NAME_BYTES {
        return Err(TableRenameIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.encode_utf16().count() > MAX_NAME_UNITS {
        return Err(TableRenameIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.chars().any(char::is_control) {
        return Err(TableRenameIntentError::InvalidRecord);
    }
    Ok(())
}

fn copy_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SchemaName, TableRenameIntentError> {
    validate_name(text, budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    value.push_str(text);
    SchemaName::new(value).map_err(|_| TableRenameIntentError::InvalidRecord)
}

#[cfg(test)]
mod tests;
