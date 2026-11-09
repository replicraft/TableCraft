//! TCQE0002 original expression requests; TCQE0001 remains a separate language.
mod codec;
mod planning;
mod recovery;
#[cfg(test)]
mod tests;

use super::{NativeIntentError, binding};
use crate::{
    InspectError, NativeImage, NativeQueryExpressionEditsPlan, NativeRecoveryQualificationError,
    OwnedQueryFragmentExpressionEdit, QueryAttribute, QueryExpressionEditsError,
    QueryFragmentExpressionEdit, header::check, read_saved_query_definition,
};
use std::io::Cursor;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use thiserror::Error;

pub use planning::{PreparedQueryExpressionEdits, RecoverableNativeQueryExpressionEditsPlan};
pub use recovery::{
    QualifiedQueryExpressionEditsRecovery, qualify_native_query_expression_edits_recovery,
};

const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();
// This is a UTF8 wire ceiling, not native Memo admission. Every preferred-inline
// native body fits it; the real native planner independently rechecks storage.
const MAX_EXPRESSION_BYTES: usize = 128;
// Persisted framing has its own closed count ceiling, independent of future
// changes to native scan quotas. Fresh reconstruction still proves actual rows.
const MAX_EDITS: usize = 16_384;

/// Refusal before native reconstruction or recovery gains operation authority.
#[derive(Debug, Error)]
pub enum QueryExpressionEditsIntentError {
    /// Native inspection, cancellation, allocation or cumulative resource limit.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The shared complete-source binding is malformed or cannot be computed.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The distinct native expression candidate profile refused the request.
    #[error(transparent)]
    Expression(#[from] QueryExpressionEditsError),
    /// Reconstructed complete images differ or exhaust their proof allowance.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// The closed original-request record has invalid framing or invariants.
    #[error("invalid native expression-edits intent record")]
    InvalidRecord,
    /// A query-action prefix selects another version, without decoder fallback.
    #[error("native expression-edits intent version is unsupported")]
    UnsupportedVersion,
    /// The request selects an unimplemented native source/request profile.
    #[error("native expression-edits intent profile is unsupported")]
    UnsupportedProfile,
    /// The complete captured BEFORE length or digest no longer matches.
    #[error("native expression-edits before image does not match")]
    BeforeMismatch,
    /// The resolved original identity or ordered requests differ from the intent.
    #[error("native expression-edits identity does not match")]
    IdentityMismatch,
}

impl From<LimitError> for QueryExpressionEditsIntentError {
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

/// Decoded original requests are data, not mutation or journal authority.
pub struct NativeQueryExpressionEditsIntent {
    identity: Identity,
    query: String,
    edits: Vec<OwnedQueryFragmentExpressionEdit>,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeQueryExpressionEditsIntent {
    /// Closed upper bound checked before persisted-record allocation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    fn capture(
        plan: &NativeQueryExpressionEditsPlan,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryExpressionEditsIntentError> {
        check(budget, cancelled)?;
        validate_name(plan.query())?;
        if plan.edits().len() > MAX_EDITS {
            return Err(QueryExpressionEditsIntentError::InvalidRecord);
        }

        let identity = read_identity(plan, budget, cancelled)?;
        budget.charge_allocation(
            plan.edits().len() as u64,
            size_of::<OwnedQueryFragmentExpressionEdit>() as u64,
        )?;
        let mut edits = Vec::new();
        edits
            .try_reserve_exact(plan.edits().len())
            .map_err(|_| InspectError::Allocation)?;
        for edit in plan.edits() {
            check(budget, cancelled)?;
            let request = edit.as_request();
            validate_edit(request.attribute, request.expected, request.replacement)?;
            edits.push(OwnedQueryFragmentExpressionEdit::copy_from_request(
                &request, budget, cancelled,
            )?);
        }

        let intent = Self {
            identity,
            query: copy_text(plan.query(), budget, cancelled)?,
            edits,
            before_length: plan.before_bytes().len() as u64,
            before_binding: binding::digest(plan.before_bytes(), budget, cancelled)?,
        };
        intent.validate(budget, cancelled)?;
        Ok(intent)
    }

    /// Original source-resolved name, without renaming or namespace reassignment.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Exact signed native query identity, without table-page masking.
    pub const fn native_id(&self) -> i32 {
        self.identity.native_id
    }

    /// Requests in original explicit placement order, not sorted physical order.
    pub fn edits(&self) -> &[OwnedQueryFragmentExpressionEdit] {
        &self.edits
    }

    /// Encode only the original request and complete-source binding.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, QueryExpressionEditsIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode the exact TCQE0002 language; never retry another action/version.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryExpressionEditsIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Bind complete original bytes before native reconstruction is attempted.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryExpressionEditsIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length
            || binding::digest(before, budget, cancelled)? != self.before_binding
        {
            return Err(QueryExpressionEditsIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn validate(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryExpressionEditsIntentError> {
        validate_name(&self.query)?;
        if self.edits.len() > MAX_EDITS {
            return Err(QueryExpressionEditsIntentError::InvalidRecord);
        }
        for (index, edit) in self.edits.iter().enumerate() {
            check(budget, cancelled)?;
            let edit = edit.as_request();
            validate_edit(edit.attribute, edit.expected, edit.replacement)?;
            budget.charge(Resource::Steps, index as u64)?;
            if self.edits[..index].iter().any(|prior| {
                let prior = prior.as_request();
                prior.attribute == edit.attribute && prior.order == edit.order
            }) {
                return Err(QueryExpressionEditsIntentError::InvalidRecord);
            }
        }
        Ok(())
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeQueryExpressionEditsPlan, QueryExpressionEditsIntentError> {
        self.validate(budget, cancelled)?;
        budget.charge_allocation(
            self.edits.len() as u64,
            size_of::<QueryFragmentExpressionEdit<'_>>() as u64,
        )?;
        let mut requests = Vec::new();
        requests
            .try_reserve_exact(self.edits.len())
            .map_err(|_| InspectError::Allocation)?;
        for edit in &self.edits {
            check(budget, cancelled)?;
            requests.push(edit.as_request());
        }
        Ok(image.plan_query_expression_edits(&self.query, &requests, budget, cancelled)?)
    }

    fn verify_plan(
        &self,
        plan: &NativeQueryExpressionEditsPlan,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryExpressionEditsIntentError> {
        if read_identity(plan, budget, cancelled)? != self.identity
            || plan.query() != self.query
            || plan.edits().len() != self.edits.len()
        {
            return Err(QueryExpressionEditsIntentError::IdentityMismatch);
        }
        for (edit, captured) in self.edits.iter().zip(plan.edits()) {
            check(budget, cancelled)?;
            let edit = edit.as_request();
            let request = captured.as_request();
            if edit.attribute != request.attribute
                || edit.order != request.order
                || edit.expected != request.expected
                || edit.replacement != request.replacement
            {
                return Err(QueryExpressionEditsIntentError::IdentityMismatch);
            }
        }
        Ok(())
    }
}

fn read_identity(
    plan: &NativeQueryExpressionEditsPlan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Identity, QueryExpressionEditsIntentError> {
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
        return Err(QueryExpressionEditsIntentError::IdentityMismatch);
    }
    Ok(Identity {
        native_id: origin.native_id(),
        parent_id: origin.parent_id(),
        catalog_page: origin.page(),
        catalog_slot: origin.slot(),
    })
}

fn validate_name(query: &str) -> Result<(), QueryExpressionEditsIntentError> {
    if query.is_empty()
        || query.len() > MAX_NAME_BYTES
        || query.encode_utf16().count() > MAX_NAME_UNITS
        || query.chars().any(char::is_control)
    {
        return Err(QueryExpressionEditsIntentError::InvalidRecord);
    }
    Ok(())
}

fn validate_edit(
    attribute: QueryAttribute,
    expected: &str,
    replacement: &str,
) -> Result<(), QueryExpressionEditsIntentError> {
    if !matches!(attribute, QueryAttribute::Column | QueryAttribute::Where)
        || expected.is_empty()
        || replacement.is_empty()
        || expected.len() > MAX_EXPRESSION_BYTES
        || replacement.len() > MAX_EXPRESSION_BYTES
    {
        return Err(QueryExpressionEditsIntentError::InvalidRecord);
    }
    Ok(())
}

fn copy_text(
    value: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, QueryExpressionEditsIntentError> {
    check(budget, cancelled)?;
    budget.charge_allocation(value.len() as u64, 1)?;
    budget.charge(Resource::Steps, value.len() as u64)?;
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    output.push_str(value);
    Ok(output)
}
