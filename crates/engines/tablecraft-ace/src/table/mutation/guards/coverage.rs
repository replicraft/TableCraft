//! Explicit logical preparation from the exact source held by an inert report.
use super::{
    BeforeChangeFieldPreparation, BeforeEventGuardError, BeforeEventGuardPreparation,
    BeforeEventGuardSelector, BeforeEventIntents, BeforeGuardUnsupported, selection,
};
use crate::{InspectError, PayloadPresence, TableExtraCoverage, TableMacroCoverage, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;

impl<'image> TableMacroCoverage<'image> {
    /// Prepare explicitly selected guards using this report's original image.
    ///
    /// Unlike the selected-subset image method, this convenience requires every
    /// observed root Extra property to be represented by the explicit selectors.
    /// Body/module content, uninspected carriers and any child remain refused.
    /// The legacy guard compiler still applies its selected-event semantics;
    /// complete property selection does not grant full dormant-action preflight.
    /// This does not discover required events, prove absence or permit a save.
    pub fn prepare_before_event_guards(
        &self,
        selectors: &[BeforeEventGuardSelector<'_>],
        original: BeforeEventIntents<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BeforeEventGuardPreparation<'image>, BeforeEventGuardError> {
        let _span = tracing::debug_span!("native.table.macro_coverage.prepare_guards").entered();
        let name = self.preparation_table_name(selectors, budget, cancelled)?;
        self.image()
            .prepare_ace16_before_event_guards(name, selectors, original, budget, cancelled)
    }

    /// Prepare complete explicitly selected BeforeChange fields on this source.
    /// Dormant supported events still undergo the existing complete preflight.
    /// The returned logical rows grant no physical or normal-save authority.
    pub fn prepare_before_change_fields(
        &self,
        selectors: &[BeforeEventGuardSelector<'_>],
        original: BeforeEventIntents<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BeforeChangeFieldPreparation<'image>, BeforeEventGuardError> {
        let _span = tracing::debug_span!("native.table.macro_coverage.prepare_fields").entered();
        let name = self.preparation_table_name(selectors, budget, cancelled)?;
        self.image()
            .prepare_ace16_before_change_fields(name, selectors, original, budget, cancelled)
    }

    fn preparation_table_name(
        &self,
        selectors: &[BeforeEventGuardSelector<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&'image str, BeforeEventGuardError> {
        check(budget, cancelled)?;
        let image = self.image();
        image.require_standard_capture()?;
        let object = self.table_origin().object_id();
        let extra = self.table_extra();
        if extra.object_id() != object {
            return Err(BeforeEventGuardError::Preservation);
        }
        if self.body().presence() != PayloadPresence::Absent
            || self.module().presence() != PayloadPresence::Absent
            || !self.children().is_empty()
        {
            return Err(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::SourceProfile,
            ));
        }
        if extra.presence() != PayloadPresence::Present
            || extra.coverage() != TableExtraCoverage::WideMr2
        {
            return Err(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Structure,
            ));
        }
        selection::validate(selectors, budget, cancelled)?;
        selection::require_complete(extra, selectors, budget, cancelled)?;

        let catalog = image.catalog().catalog();
        let mut selected = None;
        for descriptor in catalog.objects() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            if descriptor.id() == object {
                selected = Some(descriptor);
                break;
            }
        }
        let selected = selected.ok_or(InspectError::UnknownObject)?;
        if selected.kind() != ObjectKind::Table {
            return Err(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::SourceProfile,
            ));
        }
        let name = selected.name().as_str();

        // The existing preparation resolves this name through the same catalog.
        // Prepay that lookup and reject ambiguity before evaluating any program.
        let mut work = 0_u64;
        for descriptor in catalog.objects() {
            check(budget, cancelled)?;
            work = work
                .checked_add(descriptor.name().as_str().len() as u64)
                .and_then(|value| value.checked_add(name.len() as u64))
                .and_then(|value| value.checked_add(1))
                .ok_or(InspectError::StructureLimit)?;
        }
        budget
            .charge(Resource::Steps, work)
            .map_err(InspectError::from)?;
        let resolved = catalog
            .lookup(name, Some(ObjectKind::Table), None)
            .map_err(InspectError::from)?
            .ok_or(InspectError::UnknownObject)?;
        if resolved.id() != object {
            return Err(BeforeEventGuardError::Preservation);
        }
        check(budget, cancelled)?;
        Ok(name)
    }
}
