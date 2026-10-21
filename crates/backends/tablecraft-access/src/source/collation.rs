//! Explicit homogeneous native policy selection, enforced during schema discovery.
use super::NativeSource;
use crate::{GeneralCollation, NativeCollationResolver};
use std::cell::RefCell;
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
        self.check_text_policy_selection()?;
        let order = inspect_database_sort_order(
            self.source.get_mut(),
            self.budget.get_mut(),
            self.cancelled,
        )?;
        let policy = GeneralCollation::new(order, comparison_budget, self.cancelled)?;
        self.text_policy_scope = TextPolicyScope::Homogeneous(order);
        Ok((self, policy))
    }
    /// Enable explicit per-field General resolver routing before binding.
    /// The database supplies only the default descriptor; available scalar Text
    /// fields retain their exact identities. Unknown field policies fail only
    /// when a text comparison requires them; conflicting expressions fail in the
    /// query layer. Collections remain unsupported until child policies resolve.
    /// Hosts choose module-default string/LIKE policies separately. The returned
    /// resolver borrows a separate cumulative comparison quota and shares the
    /// source cancellation callback. This is not native SQL qualification.
    pub fn with_database_collation_resolver<'b>(
        mut self,
        comparison_budget: &'b RefCell<ResourceBudget>,
    ) -> Result<(Self, NativeCollationResolver<'b>), InspectError>
    where
        'a: 'b,
    {
        let _span = tracing::debug_span!("native.query.collation.configure").entered();
        self.check_text_policy_selection()?;
        let order = inspect_database_sort_order(
            self.source.get_mut(),
            self.budget.get_mut(),
            self.cancelled,
        )?;
        let resolver = NativeCollationResolver::new(order, comparison_budget, self.cancelled)?;
        self.text_policy_scope = TextPolicyScope::PerField;
        Ok((self, resolver))
    }
    fn check_text_policy_selection(&mut self) -> Result<(), InspectError> {
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
        if self.text_policy_scope.selected() || self.multivalues || self.collections {
            return Err(InspectError::Unsupported);
        }
        Ok(())
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

pub(super) fn text_collation(field: &tablecraft_ace::NativeColumn) -> Option<TextCollationId> {
    field.text_sort_order().map(crate::collation::identity)
}

#[derive(Clone, Copy)]
pub(super) enum TextPolicyScope {
    Unselected,
    Homogeneous(NativeSortOrder),
    PerField,
}
impl TextPolicyScope {
    pub(super) fn selected(self) -> bool {
        !matches!(self, Self::Unselected)
    }
    pub(super) fn required_order(self) -> Option<NativeSortOrder> {
        if let Self::Homogeneous(order) = self {
            Some(order)
        } else {
            None
        }
    }
}
