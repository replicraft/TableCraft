//! A separate topology-preserving SELECT expression candidate language.
//! Existing fitting plans and TCQE0001 reconstruction remain unchanged.
mod definition;
#[cfg(test)]
mod tests;

use crate::{
    FileFormat, InspectError, NativeImage, QueryAttribute, header::check,
    read_saved_query_definition,
};
use std::io::Cursor;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

/// Refusal from the separate native topology-preserving expression profile.
#[derive(Debug, Error)]
pub enum QueryExpressionEditsError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error("native SELECT expression source or storage is unsupported")]
    Source,
    #[error("native SELECT expression request or fragment topology is unsupported")]
    Definition,
    #[error("native SELECT expression storage capacity is unsupported")]
    Capacity,
    #[error("native SELECT expression preservation proof failed")]
    Preservation,
}

impl From<LimitError> for QueryExpressionEditsError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// One exact original native expression slot; no ROW address or final bytes.
pub struct QueryFragmentExpressionEdit<'a> {
    pub attribute: QueryAttribute,
    pub order: [u8; size_of::<u32>()],
    pub expected: &'a str,
    pub replacement: &'a str,
}

/// Captured semantic request; no serializer or arbitrary candidate constructor.
pub struct OwnedQueryFragmentExpressionEdit {
    attribute: QueryAttribute,
    order: [u8; size_of::<u32>()],
    expected: String,
    replacement: String,
}

impl OwnedQueryFragmentExpressionEdit {
    pub fn as_request(&self) -> QueryFragmentExpressionEdit<'_> {
        QueryFragmentExpressionEdit {
            attribute: self.attribute,
            order: self.order,
            expected: &self.expected,
            replacement: &self.replacement,
        }
    }
}

/// Complete native candidate only; no intent/journal/adapter/runtime permission.
pub struct NativeQueryExpressionEditsPlan {
    before: NativeImage,
    after: Vec<u8>,
    query: String,
    native_id: i32,
    edits: Vec<OwnedQueryFragmentExpressionEdit>,
}

impl NativeImage {
    /// Replace existing projection/WHERE expressions in an uncompiled local SELECT.
    ///
    /// Native query identity, all fragment keys/order/membership and catalog
    /// payloads remain unchanged. Multiple definitions/DATA pages are admitted
    /// only after complete storage/index/ownership proof. Native inline Memo
    /// placement may retain retired hidden rows; no SQL or active content runs.
    /// Edited Memo bodies must retain preferred inline storage; unedited external
    /// bodies remain protected and unchanged. Cross-page growth, topology changes
    /// and compiled payloads refuse. All requested edits are planned atomically,
    /// without partial output.
    pub fn plan_query_expression_edits(
        self,
        query: &str,
        edits: &[QueryFragmentExpressionEdit<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeQueryExpressionEditsPlan, QueryExpressionEditsError> {
        let _span = tracing::debug_span!("native.query_expression_edits.plan").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;

        let header = self.catalog().header();
        if header.format() != FileFormat::Ace2016
            || header.subversion() != 0
            || header.encoded_pages()
        {
            return Err(QueryExpressionEditsError::Source);
        }

        let scope = self.catalog().catalog().scope();
        let definition =
            read_saved_query_definition(&mut self.reader(), scope, query, budget, cancelled)?;
        definition::qualify(&definition, edits, budget, cancelled)?;

        let after = crate::table::mutation::query_expression_edit::build(
            &self,
            definition.origin(),
            edits,
            budget,
            cancelled,
        )?;

        let candidate = NativeImage::capture(&mut Cursor::new(&after), scope, budget, cancelled)?;
        let fresh =
            read_saved_query_definition(&mut candidate.reader(), scope, query, budget, cancelled)?;
        definition::verify(&definition, &fresh, edits, budget, cancelled)?;
        crate::table::mutation::query_expression_edit::verify(
            &self,
            &candidate,
            definition.origin(),
            fresh.origin(),
            edits,
            budget,
            cancelled,
        )?;

        let name = copy_text(definition.object().name().as_str(), budget, cancelled)?;
        let native_id = definition.origin().native_id();

        budget.charge_allocation(
            edits.len() as u64,
            size_of::<OwnedQueryFragmentExpressionEdit>() as u64,
        )?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(edits.len())
            .map_err(|_| InspectError::Allocation)?;
        for edit in edits {
            check(budget, cancelled)?;
            owned.push(OwnedQueryFragmentExpressionEdit {
                attribute: edit.attribute,
                order: edit.order,
                expected: copy_text(edit.expected, budget, cancelled)?,
                replacement: copy_text(edit.replacement, budget, cancelled)?,
            });
        }

        Ok(NativeQueryExpressionEditsPlan {
            before: self,
            after,
            query: name,
            native_id,
            edits: owned,
        })
    }
}

impl NativeQueryExpressionEditsPlan {
    /// Complete immutable source retained during qualification.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete unpublished candidate; not a writable-session capability.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Original source-resolved query name.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Exact signed native query identity, without table-page masking.
    pub const fn native_id(&self) -> i32 {
        self.native_id
    }

    /// Original captured requests in explicit placement order.
    pub fn edits(&self) -> &[OwnedQueryFragmentExpressionEdit] {
        &self.edits
    }

    /// Exact no-ops retain all source and request qualification.
    pub fn changed(&self) -> bool {
        self.edits
            .iter()
            .any(|edit| edit.expected != edit.replacement)
    }
}

fn copy_text(
    value: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, InspectError> {
    check(budget, cancelled)?;
    budget.charge_allocation(value.len() as u64, 1)?;
    budget.charge(Resource::Steps, value.len() as u64)?;

    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    text.push_str(value);
    Ok(text)
}
