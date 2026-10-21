//! Explicit homogeneous native policy selection, enforced during schema discovery.
use super::NativeSource;
use crate::GeneralCollation;
use tablecraft_ace::{InspectError, NativeSortOrder, TableSchema, inspect_database_sort_order};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::source::TextCollationId;
use tablecraft_schema::ScalarType;

impl<'a, R: tablecraft_ace::NativeReadSource> NativeSource<'a, R> {
    /// Select the database's stored General policy before binding any queries.
    ///
    /// Returns the same source and a separately budgeted query policy. Every
    /// subsequently discovered scalar table schema must have the same exact
    /// descriptor on all available scalar Text fields, including unprojected fields. Unknown and
    /// mixed orders fail without guessing, record scans or ordinal fallback.
    /// This conservative homogeneous scope is not per-expression resolution or
    /// native SQL qualification. Explicit family overrides remain separate.
    ///
    /// Call before schema discovery; collection opt-ins are currently unsupported
    /// because their child text descriptors need independent resolution. Later
    /// collection opt-ins also fail when their metadata is requested. The policy
    /// shares this source's cancellation callback, but not its I/O allowance.
    /// Hosts still supply module-default string and LIKE syntax policies.
    pub fn with_database_collation(
        mut self,
        comparison_budget: ResourceBudget,
    ) -> Result<(Self, GeneralCollation<'a>), InspectError> {
        if (self.cancelled)() {
            return Err(InspectError::Cancelled);
        }
        for entry in &self.entries {
            if (self.cancelled)() {
                return Err(InspectError::Cancelled);
            }
            self.budget.get_mut().charge(Resource::Steps, 1)?;
            if entry.columns.get().is_some() {
                return Err(InspectError::Unsupported);
            }
        }
        if self.database_order.is_some() || self.multivalues || self.collections {
            return Err(InspectError::Unsupported);
        }
        let order = inspect_database_sort_order(
            self.source.get_mut(),
            self.budget.get_mut(),
            self.cancelled,
        )?;
        let policy = GeneralCollation::new(order, comparison_budget, self.cancelled)?;
        self.database_order = Some(order);
        Ok((self, policy))
    }
}

pub(super) fn validate(
    schema: &TableSchema,
    order: Option<NativeSortOrder>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let Some(order) = order else { return Ok(()) };
    for column in schema.columns() {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        if column.scalar_type() == Some(ScalarType::Text) && column.text_sort_order() != Some(order)
        {
            return Err(InspectError::Unsupported);
        }
    }
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    Ok(())
}

// Only this adapter interprets the exact native descriptor. The query layer
// receives an opaque source-scoped identity, including unknown native orders.
pub(super) fn text_collation(field: &tablecraft_ace::NativeColumn) -> Option<TextCollationId> {
    field.text_sort_order().map(|order| {
        let locale = order.lcid().to_le_bytes();
        TextCollationId::from_host(u64::from(u32::from_le_bytes([
            locale[0],
            locale[1],
            order.variant(),
            order.version(),
        ])))
    })
}
