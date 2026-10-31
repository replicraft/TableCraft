//! Native source-reference binding and selector-free BeforeChange preparation.
mod binding;

#[cfg(test)]
mod originals;

use super::{
    BeforeChangeFieldPreparation, BeforeEvent, BeforeEventGuardError, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardUnsupported, buffer,
};
use crate::{NativeImage, header::check};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

fn unresolved() -> BeforeEventGuardError {
    BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::NativeCoverage)
}

fn unsupported_event() -> BeforeEventGuardError {
    BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::NativeEvent)
}

impl NativeImage {
    /// Prepare UPDATE BeforeChange fields from native-owned MR2 event references.
    ///
    /// The source table identity and stored XML Event declarations determine the
    /// selection; property names are never interpreted as event names. Every
    /// observed property must bind and undergo the complete compiler preflight.
    /// Names-only, opaque or incomplete carrier coverage remains unresolved.
    /// Required after-effects and named macros remain unsupported, even for no
    /// rows. This returns logical preparation, not absence or save permission.
    pub fn prepare_native_before_change_fields(
        &self,
        table: ObjectId,
        original: BeforeEventIntents<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BeforeChangeFieldPreparation<'_>, BeforeEventGuardError> {
        let _span = tracing::debug_span!("native.before_change.native_bind.prepare").entered();
        check(budget, cancelled)?;
        if original.event() != BeforeEvent::Update {
            return Err(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Context,
            ));
        }
        let bound = binding::capture(self, table, budget, cancelled)?;
        if bound.unresolved {
            return Err(unresolved());
        }
        let mut has_change = false;
        for reference in &bound.references {
            check(budget, cancelled)?;
            match reference.event {
                binding::Event::BeforeChange => has_change = true,
                binding::Event::BeforeDelete => {}
                _ => return Err(unsupported_event()),
            }
        }
        if !has_change {
            return Err(unresolved());
        }
        let extra = bound.coverage.table_extra();
        let mut selectors = buffer::vector(bound.references.len(), budget, cancelled)?;
        for reference in &bound.references {
            check(budget, cancelled)?;
            let group = extra.groups().get(reference.group).ok_or_else(unresolved)?;
            let property = group
                .properties()
                .and_then(|properties| properties.get(reference.property))
                .ok_or_else(unresolved)?;
            selectors.push(BeforeEventGuardSelector::new(group.kind(), property.name()));
        }
        // The existing source-bound convenience checks exact scoped/name
        // identity, complete selectors, and compiles dormant definitions before
        // evaluation. The returned lifetime belongs to self, not local metadata.
        bound
            .coverage
            .prepare_before_change_fields(&selectors, original, budget, cancelled)
    }
}
