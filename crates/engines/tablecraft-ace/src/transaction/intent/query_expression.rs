//! Original fitting query requests bound to a complete native source.
mod codec;
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
use crate::{
    InspectError, NativeImage, NativeQueryExpressionPlan, QueryExpressionError, header::check,
    read_saved_query_definition,
};
use std::io::Cursor;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use thiserror::Error;

pub use planning::{
    PreparedQueryExpression, QueryExpressionRecoveryError, RecoverableNativeQueryExpressionPlan,
    RecoverableQueryExpressionError,
};
pub use recovery::{QualifiedQueryExpressionRecovery, qualify_native_query_expression_recovery};

#[cfg(test)]
mod tests;

const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();
const MAX_LITERAL_BYTES: usize = 10;

/// Closed query-action data failure, never native mutation authority.
#[derive(Debug, Error)]
pub enum QueryExpressionIntentError {
    /// Bounded inspection, cancellation, allocation or resource exhaustion.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The common complete-source binding contract refused its input.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// Framing, identity or original-request invariants are malformed.
    #[error("invalid native query-expression intent record")]
    InvalidRecord,
    /// The query-action prefix is recognized but its version is unsupported.
    #[error("native query-expression intent version is unsupported")]
    UnsupportedVersion,
    /// The record selects an unimplemented native source/request profile.
    #[error("native query-expression intent profile is unsupported")]
    UnsupportedProfile,
    /// The original complete source length or digest changed.
    #[error("native query-expression intent before image does not match")]
    BeforeMismatch,
    /// Fresh identity or original/proposed expression differs from the request.
    #[error("native query-expression intent identity does not match")]
    IdentityMismatch,
}

impl From<LimitError> for QueryExpressionIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    native_id: i32,
    parent_id: i32,
    catalog_page: u32,
    catalog_slot: u8,
}

/// Exact original name/native identity and literal request with a BEFORE binding.
/// Decoding it does not qualify storage, a replacement image or a journal.
pub struct NativeQueryExpressionIntent {
    identity: Identity,
    query: String,
    expected: String,
    replacement: String,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeQueryExpressionIntent {
    /// Closed upper bound, checked before allocating a persisted record.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    fn capture(
        plan: &NativeQueryExpressionPlan,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryExpressionIntentError> {
        check(budget, cancelled)?;
        validate_request(
            plan.query(),
            plan.expected_expression(),
            plan.replacement_expression(),
        )?;
        let identity = read_identity(plan, budget, cancelled)?;
        let query = copy_text(plan.query(), budget)?;
        let expected = copy_text(plan.expected_expression(), budget)?;
        let replacement = copy_text(plan.replacement_expression(), budget)?;
        validate_request(&query, &expected, &replacement)?;
        let before_binding = binding::digest(plan.before_bytes(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            identity,
            query,
            expected,
            replacement,
            before_length: plan.before_bytes().len() as u64,
            before_binding,
        })
    }

    /// Original catalog spelling, without rename or namespace reassignment.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Exact original stored literal, never an evaluated expression result.
    pub fn expected_expression(&self) -> &str {
        &self.expected
    }

    /// Exact proposed literal, never replacement rows or patched AFTER bytes.
    pub fn replacement_expression(&self) -> &str {
        &self.replacement
    }

    /// Exact signed native query identity, without table/page masking.
    pub const fn native_id(&self) -> i32 {
        self.identity.native_id
    }

    /// Encode only the closed original request and full-source binding.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, QueryExpressionIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Validate complete framing before copying strings; no other action fallback.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryExpressionIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Recheck the complete original bytes before reconstructing native authority.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryExpressionIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(QueryExpressionIntentError::BeforeMismatch);
        }
        if binding::digest(before, budget, cancelled)? != self.before_binding {
            return Err(QueryExpressionIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeQueryExpressionPlan, QueryExpressionError> {
        image.plan_query_literal_expression(
            self.query(),
            self.expected_expression(),
            self.replacement_expression(),
            budget,
            cancelled,
        )
    }

    fn verify_plan(
        &self,
        plan: &NativeQueryExpressionPlan,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryExpressionIntentError> {
        if read_identity(plan, budget, cancelled)? != self.identity
            || plan.query() != self.query()
            || plan.expected_expression() != self.expected_expression()
            || plan.replacement_expression() != self.replacement_expression()
        {
            return Err(QueryExpressionIntentError::IdentityMismatch);
        }
        Ok(())
    }
}

fn read_identity(
    plan: &NativeQueryExpressionPlan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Identity, QueryExpressionIntentError> {
    let definition = read_saved_query_definition(
        &mut Cursor::new(plan.before_bytes()),
        CatalogScope::new(1),
        plan.query(),
        budget,
        cancelled,
    )?;
    let origin = definition.origin();
    if origin.native_id() != plan.native_id() || definition.object().name().as_str() != plan.query()
    {
        return Err(QueryExpressionIntentError::IdentityMismatch);
    }
    Ok(Identity {
        native_id: origin.native_id(),
        parent_id: origin.parent_id(),
        catalog_page: origin.page(),
        catalog_slot: origin.slot(),
    })
}

fn validate_request(
    query: &str,
    expected: &str,
    replacement: &str,
) -> Result<(), QueryExpressionIntentError> {
    if query.is_empty()
        || query.len() > MAX_NAME_BYTES
        || query.encode_utf16().count() > MAX_NAME_UNITS
        || query.chars().any(char::is_control)
        || expected.len() != replacement.len()
    {
        return Err(QueryExpressionIntentError::InvalidRecord);
    }
    for literal in [expected, replacement] {
        if literal.is_empty()
            || literal.len() > MAX_LITERAL_BYTES
            || !literal.bytes().all(|byte| byte.is_ascii_digit())
            || (literal.len() > 1 && literal.starts_with('0'))
            || literal.parse::<i32>().is_err()
        {
            return Err(QueryExpressionIntentError::InvalidRecord);
        }
    }
    Ok(())
}

fn copy_text(
    value: &str,
    budget: &mut ResourceBudget,
) -> Result<String, QueryExpressionIntentError> {
    budget.charge_allocation(value.len() as u64, 1)?;
    budget.charge(Resource::Steps, value.len() as u64)?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    result.push_str(value);
    Ok(result)
}
