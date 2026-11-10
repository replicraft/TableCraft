//! Original AddTable declarations bound to actual populated source bytes, never permission.
mod codec;
mod recovery;
mod write;

use super::{FirstTableIntentError, NativeIntentError, binding};
use crate::{AddTableSourceProfile, FirstTableDefinition, InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

pub use recovery::{QualifiedAddTableRecovery, qualify_native_add_table_recovery};
pub use write::PreparedAddTable;

type BeforeBinding = [u8; 32];

/// Original AddTable framing, declaration-template or actual source-binding failure.
#[derive(Debug, Error)]
pub enum AddTableIntentError {
    /// Resource, cancellation or allocation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Source hashing or binding-service failure.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The closed declaration template failed validation.
    #[error(transparent)]
    Declaration(#[from] FirstTableIntentError),
    /// Malformed framing, length, flags or reserved bytes.
    #[error("invalid add-table intent record")]
    InvalidRecord,
    /// The outer action or nested template version is unsupported.
    #[error("add-table intent version is unsupported")]
    UnsupportedVersion,
    /// The pinned outer or nested construction profile is unsupported.
    #[error("add-table construction profile is unsupported")]
    UnsupportedProfile,
    /// The actual complete original image does not match its retained binding.
    #[error("add-table intent before image does not match")]
    BeforeMismatch,
}

/// Exact original schema and complete source binding for TCAT0001 through TCAT0005.
/// TCAT1 binds exactly ten pages; TCAT2 binds thirteen through sixty-one pages
/// in three-page steps. TCAT3/4 retain those respective source partitions but
/// require described TCNT5 templates. TCAT5 retains a distinct mixed-ledger identity
/// and closed TCNT1–6 declaration grammar. These wire bounds grant no native role authority.
/// Its nested TCNT record is only a closed declaration template. The template's
/// canonical-blank header never identifies or authorizes this operation's source.
/// TCAT7 independently retains native declarations and the actual Mixed/Retained
/// source cohort. Its nested TCNT7 never promotes physical source authority.
/// Neither capture nor decode qualifies native mutation, journaling or recovery.
pub struct NativeAddTableIntent {
    definition: FirstTableDefinition,
    profile: codec::Profile,
    before_length: u64,
    before_binding: BeforeBinding,
}

impl NativeAddTableIntent {
    /// Maximum complete record checked before decoded declaration allocations.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Maximum native declaration record, derived from the native field count.
    pub const MAX_NATIVE_ENCODED_BYTES: usize = codec::NATIVE_MAX_RECORD_BYTES;

    /// Bind an already checked declaration to the actual source's complete bytes.
    /// Native source roles, capacity and name conflicts still require a fresh plan.
    pub fn new(
        definition: FirstTableDefinition,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableIntentError> {
        check(budget, cancelled)?;
        if definition.is_native() {
            return Err(AddTableIntentError::UnsupportedProfile);
        }
        let profile = codec::legacy_profile(before.len() as u64)
            .ok_or(AddTableIntentError::BeforeMismatch)?;
        let before_binding = binding::digest(before, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            definition,
            profile,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Bind a declaration using the explicit mixed-ledger TCAT5 language.
    /// This data-only constructor does not classify or authorize native source roles.
    pub fn new_mixed(
        definition: FirstTableDefinition,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableIntentError> {
        check(budget, cancelled)?;
        if definition.is_native() {
            return Err(AddTableIntentError::UnsupportedProfile);
        }
        if !codec::mixed_before_length(before.len() as u64, definition.primary_position().is_none())
        {
            return Err(AddTableIntentError::BeforeMismatch);
        }
        let before_binding = binding::digest(before, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            definition,
            profile: codec::Profile::Mixed,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Bind a declaration to the retained-source TCAT6 family without granting a plan.
    pub fn new_retained(
        definition: FirstTableDefinition,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableIntentError> {
        check(budget, cancelled)?;
        if definition.is_native() {
            return Err(AddTableIntentError::UnsupportedProfile);
        }
        if !codec::retained_before_length(before.len() as u64) {
            return Err(AddTableIntentError::BeforeMismatch);
        }
        let profile = codec::Profile::Retained;
        let before_binding = profile.digest(before, budget, cancelled)?;
        Ok(Self {
            definition,
            profile,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Bind a native declaration to actual retained source bytes as TCAT7 data.
    /// Source roles and complete reconstruction remain independently required.
    pub fn new_native(
        definition: FirstTableDefinition,
        source_profile: AddTableSourceProfile,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableIntentError> {
        check(budget, cancelled)?;
        if !definition.is_native() {
            return Err(AddTableIntentError::UnsupportedProfile);
        }
        let profile = match source_profile {
            AddTableSourceProfile::Mixed => codec::Profile::NativeMixed,
            AddTableSourceProfile::Retained => codec::Profile::NativeRetained,
            AddTableSourceProfile::LegacyGenerated => {
                return Err(AddTableIntentError::UnsupportedProfile);
            }
        };
        if !codec::retained_before_length(before.len() as u64) {
            return Err(AddTableIntentError::BeforeMismatch);
        }
        let before_binding = profile.digest(before, budget, cancelled)?;
        Ok(Self {
            definition,
            profile,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Whether the original request belongs to the native TCAT7 declaration family.
    pub fn is_native(&self) -> bool {
        self.profile.is_native()
    }

    /// Closed construction family retained by the original request.
    pub fn source_profile(&self) -> AddTableSourceProfile {
        match self.profile {
            codec::Profile::Retained | codec::Profile::NativeRetained => {
                AddTableSourceProfile::Retained
            }
            codec::Profile::Mixed | codec::Profile::NativeMixed => AddTableSourceProfile::Mixed,
            codec::Profile::Legacy
            | codec::Profile::Repeated
            | codec::Profile::Described
            | codec::Profile::RepeatedDescribed => AddTableSourceProfile::LegacyGenerated,
        }
    }

    /// Exact original table name; never SQL or a target path.
    pub fn table(&self) -> &str {
        self.definition.table()
    }

    /// Number of original declared fields, never a count of inserted records.
    pub fn column_count(&self) -> usize {
        self.definition.column_count()
    }

    /// Encode original declarations and actual source binding, without native planning.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, AddTableIntentError> {
        codec::encode(
            &self.definition,
            self.profile,
            self.before_length,
            self.before_binding,
            budget,
            cancelled,
        )
    }

    /// Decode only this action and its pinned template versions; no decoder fallback.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableIntentError> {
        codec::decode(raw, budget, cancelled)
    }

    /// Decode only TCAT6 retained-source data, without retrying older families.
    pub fn decode_retained(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableIntentError> {
        codec::decode_retained(raw, budget, cancelled)
    }

    /// Decode only TCAT7 with its exact native TCNT7 declaration template.
    pub fn decode_native(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddTableIntentError> {
        codec::decode_native(raw, budget, cancelled)
    }

    /// Check the actual full original image, including opaque data and free gaps.
    /// A matching digest still grants no native source-role or write permission.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), AddTableIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(AddTableIntentError::BeforeMismatch);
        }
        let actual = self.profile.digest(before, budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(AddTableIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
