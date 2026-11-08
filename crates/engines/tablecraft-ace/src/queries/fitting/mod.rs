//! Existing constant SELECT candidates with fixed identity and native topology.
use crate::{
    FileFormat, InspectError, NativeImage, QueryAttribute, SavedQueryDefinition, header::check,
    read_saved_query_definition,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_model::ObjectKind;
use thiserror::Error;

#[cfg(test)]
mod tests;

const MAX_LITERAL_BYTES: usize = 10;

/// Refusal before producing an unpublished native query-expression candidate.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueryExpressionError {
    /// Native inspection, allocation, cancellation or resource exhaustion.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The original source, physical ownership or compiled payload is unqualified.
    #[error("native query-expression source is unsupported")]
    Source,
    /// Only the observed source-free, five-fragment constant SELECT is admitted.
    #[error("native query-expression definition is unsupported")]
    Definition,
    /// The request is outside the nonnegative Int32 decimal-literal profile.
    #[error("query expression requires a canonical nonnegative Int32 literal")]
    Literal,
    /// The replacement cannot retain its exact original inline payload length.
    #[error("query expression cannot retain its existing storage span")]
    Capacity,
    /// Complete reconstruction or exact complement verification failed.
    #[error("native query-expression preservation proof failed")]
    Preservation,
}

impl From<LimitError> for QueryExpressionError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// An owning candidate for an existing definition, without journal/save authority.
///
/// Names, IDs, indexes, timestamps, properties and fragment topology are retained.
/// This type cannot create a query or authorize a normal native save/recovery.
/// The initial profile admits one five-fragment, source-free SELECT with the
/// observed `Answer` alias and an equally sized nonnegative Int32 literal.
pub struct NativeQueryExpressionPlan {
    before: NativeImage,
    after: Vec<u8>,
    query: String,
    native_id: i32,
    expected: String,
    replacement: String,
}

impl NativeImage {
    /// Build an unpublished, topology-preserving existing constant SELECT candidate.
    /// No SQL, parameter, provider, expression, macro or VBA execution occurs.
    pub fn plan_query_literal_expression(
        self,
        query: &str,
        expected: &str,
        replacement: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeQueryExpressionPlan, QueryExpressionError> {
        let _span = tracing::debug_span!("native.query_expression.plan").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        literal(expected)?;
        literal(replacement)?;
        if expected.len() != replacement.len() {
            return Err(QueryExpressionError::Capacity);
        }
        let header = self.catalog().header();
        if header.format() != FileFormat::Ace2016
            || header.subversion() != 0
            || header.encoded_pages()
        {
            return Err(QueryExpressionError::Source);
        }
        let scope = self.catalog().catalog().scope();
        let definition =
            read_saved_query_definition(&mut self.reader(), scope, query, budget, cancelled)?;
        qualify_definition(&definition, expected)?;
        budget.charge(
            Resource::Steps,
            self.catalog().catalog().objects().len() as u64,
        )?;
        let query_count = self
            .catalog()
            .catalog()
            .objects()
            .iter()
            .filter(|object| object.kind() == ObjectKind::Query)
            .count();
        if query_count != 1 {
            return Err(QueryExpressionError::Source);
        }
        let span = crate::table::mutation::query_expression::qualify(
            &self,
            definition.origin(),
            expected,
            budget,
            cancelled,
        )?;
        let name = copy_text(definition.object().name().as_str(), budget)?;
        let native_id = definition.origin().native_id();
        let expected = copy_text(expected, budget)?;
        let replacement = copy_text(replacement, budget)?;
        let length = self.as_bytes().len();
        budget.charge_allocation(length as u64, 1)?;
        budget.charge(Resource::Steps, length as u64)?;
        let mut after = Vec::new();
        after
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        after.extend_from_slice(self.as_bytes());
        for (pair, value) in after[span.clone()]
            .chunks_exact_mut(2)
            .zip(replacement.bytes())
        {
            check(budget, cancelled)?;
            pair.copy_from_slice(&u16::from(value).to_le_bytes());
        }
        for (position, (old, new)) in self.as_bytes().iter().zip(&after).enumerate() {
            check(budget, cancelled)?;
            if !span.contains(&position) && old != new {
                return Err(QueryExpressionError::Preservation);
            }
        }
        let candidate =
            NativeImage::capture(&mut std::io::Cursor::new(&after), scope, budget, cancelled)?;
        let fresh =
            read_saved_query_definition(&mut candidate.reader(), scope, &name, budget, cancelled)?;
        qualify_definition(&fresh, &replacement)?;
        if fresh.origin().native_id() != native_id
            || fresh.origin().parent_id() != definition.origin().parent_id()
            || fresh.object().name() != definition.object().name()
            || crate::table::mutation::query_expression::qualify(
                &candidate,
                fresh.origin(),
                &replacement,
                budget,
                cancelled,
            )? != span
        {
            return Err(QueryExpressionError::Preservation);
        }
        check(budget, cancelled)?;
        Ok(NativeQueryExpressionPlan {
            before: self,
            after,
            query: name,
            native_id,
            expected,
            replacement,
        })
    }
}

impl NativeQueryExpressionPlan {
    /// Exact immutable source retained throughout native qualification.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }
    /// Complete unpublished candidate; this is not a save/recovery capability.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }
    /// Original query name, without renaming or namespace reassignment.
    pub fn query(&self) -> &str {
        &self.query
    }
    /// Exact signed native query identity, unrelated to a table definition page.
    pub const fn native_id(&self) -> i32 {
        self.native_id
    }
    /// Exact original stored literal, without expression evaluation.
    pub fn expected_expression(&self) -> &str {
        &self.expected
    }
    /// Exact proposed stored literal, without expression evaluation.
    pub fn replacement_expression(&self) -> &str {
        &self.replacement
    }
    /// Exact no-ops still undergo complete source and candidate qualification.
    pub fn changed(&self) -> bool {
        self.expected != self.replacement
    }
}

fn literal(value: &str) -> Result<(), QueryExpressionError> {
    if value.is_empty()
        || value.len() > MAX_LITERAL_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
        || value.parse::<i32>().is_err()
    {
        return Err(QueryExpressionError::Literal);
    }
    Ok(())
}

fn copy_text(value: &str, budget: &mut ResourceBudget) -> Result<String, InspectError> {
    budget.charge_allocation(value.len() as u64, 1)?;
    budget.charge(Resource::Steps, value.len() as u64)?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    result.push_str(value);
    Ok(result)
}

fn qualify_definition(
    definition: &SavedQueryDefinition,
    expected: &str,
) -> Result<(), QueryExpressionError> {
    let rows = definition.rows();
    if definition.origin().flags() != 0 || rows.len() != 5 {
        return Err(QueryExpressionError::Definition);
    }
    let attributes = [
        QueryAttribute::Start,
        QueryAttribute::Type,
        QueryAttribute::Flags,
        QueryAttribute::Column,
        QueryAttribute::End,
    ];
    for (row, attribute) in rows.iter().zip(attributes) {
        let column = attribute == QueryAttribute::Column;
        let flag = match attribute {
            QueryAttribute::Type => Some(1),
            QueryAttribute::Flags => Some(0),
            _ => None,
        };
        if row.native_object_id() != definition.origin().native_id()
            || row.attribute() != Some(attribute)
            || row.flag() != flag
            || row.extra().is_some()
            || row.name2().is_some()
            || row.order() != Some([0, 0, 0, 1].as_slice())
            || row.expression() != column.then_some(expected)
            || row.name1() != column.then_some("Answer")
        {
            return Err(QueryExpressionError::Definition);
        }
    }
    Ok(())
}
