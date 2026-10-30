use super::binding;
use crate::{EditError, InspectError, Int32Update, header::check, limits::MAX_ENCODED_TEXT_BYTES};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub(super) const MAX_NAME_BYTES: usize = MAX_ENCODED_TEXT_BYTES * char::MAX.len_utf8();
const NAME_CHECK_SCALARS: usize = 4096;

/// Invalid request data, source binding or bounded processing failure.
/// No diagnostic includes names, values, paths or image contents.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativeIntentError {
    /// Resource, allocation, cancellation or shared name-validation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The existing native request count/uniqueness contract refused the data.
    #[error(transparent)]
    Request(#[from] EditError),
    /// Invalid framing, lengths, names, flags or source-image size.
    #[error("invalid native Int32 intent record")]
    InvalidRecord,
    /// The version is not implemented; no fallback or upgrade is attempted.
    #[error("native Int32 intent version is unsupported")]
    UnsupportedVersion,
    /// The requested native operation profile is not implemented by this type.
    #[error("native Int32 intent profile is unsupported")]
    UnsupportedProfile,
    /// The exact original image does not match the intent's length and binding.
    #[error("native Int32 intent before image does not match")]
    BeforeMismatch,
}

/// Original resolved Int32 requests and an exact before-image binding.
/// Constructing or decoding this data grants no write/recovery permission.
/// Names and request order/no-ops are retained without implicit normalization.
pub struct NativeInt32EditIntent {
    pub(super) table: SchemaName,
    pub(super) column: SchemaName,
    pub(super) updates: Vec<Int32Update>,
    pub(super) before_length: u64,
    pub(super) before_binding: [u8; 32],
}

impl NativeInt32EditIntent {
    /// Maximum complete encoded record, for bounded host intent capture before
    /// allocating native-agnostic journal-bundle storage.
    pub const MAX_ENCODED_BYTES: usize = super::codec::MAX_RECORD_BYTES;

    /// Bind original requests to supplied bytes. The request Vec is moved as-is;
    /// the caller owns its existing allocation. Only name copies/new work are paid.
    /// This validates request data, not native file validity or row membership.
    pub fn new(
        table: &str,
        column: &str,
        updates: Vec<Int32Update>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let mut intent = Self::from_decoded(
            table,
            column,
            updates,
            before.len() as u64,
            [0; 32],
            budget,
            cancelled,
        )?;
        intent.before_binding = binding::digest(before, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(intent)
    }

    pub(super) fn from_decoded(
        table: &str,
        column: &str,
        updates: Vec<Int32Update>,
        before_length: u64,
        before_binding: [u8; 32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        binding::length(before_length)?;
        validate_name(table, budget, cancelled)?;
        validate_name(column, budget, cancelled)?;
        Int32Update::validate_intent_requests(&updates, budget, cancelled)?;
        let table = copy_name(table, budget)?;
        let column = copy_name(column, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            column,
            updates,
            before_length,
            before_binding,
        })
    }

    /// Original table spelling, not SQL text or a file selector.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Original column spelling, resolved only by the qualified native planner.
    pub fn column(&self) -> &str {
        self.column.as_str()
    }

    /// Original written order and values, including no-op requests.
    pub fn updates(&self) -> &[Int32Update] {
        &self.updates
    }

    /// Original byte length; this alone establishes no file identity or validity.
    pub const fn before_length(&self) -> u64 {
        self.before_length
    }

    /// Verify exact source bytes before qualified native replay/recovery.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(NativeIntentError::BeforeMismatch);
        }
        let actual = binding::digest(before, budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(NativeIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

pub(super) fn validate_name(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(NativeIntentError::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, name.len() as u64)
        .map_err(InspectError::from)?;
    for (position, character) in name.chars().enumerate() {
        if position % NAME_CHECK_SCALARS == 0 {
            check(budget, cancelled)?;
        }
        if character.is_control() {
            return Err(NativeIntentError::InvalidRecord);
        }
    }
    Ok(())
}

fn copy_name(name: &str, budget: &mut ResourceBudget) -> Result<SchemaName, NativeIntentError> {
    budget
        .charge_allocation(name.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, name.len() as u64)
        .map_err(InspectError::from)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    // SchemaName performs the shared scalar-invariant pass over the owned copy.
    budget
        .charge(Resource::Steps, name.len() as u64)
        .map_err(InspectError::from)?;
    SchemaName::new(owned)
        .map_err(InspectError::from)
        .map_err(Into::into)
}
