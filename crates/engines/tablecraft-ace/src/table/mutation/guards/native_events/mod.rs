//! Native source-reference binding and selector-free BeforeChange preparation.
mod binding;

#[cfg(test)]
mod originals;

#[cfg(feature = "journal")]
use super::fields::batch::SelectedPrograms;
use super::{
    BeforeChangeFieldPreparation, BeforeEvent, BeforeEventGuardError, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardUnsupported, buffer,
};
#[cfg(feature = "journal")]
use super::{error, expression, resolver::Fields, selection};
#[cfg(feature = "journal")]
use crate::{InspectError, MacroProgramNodeKind, MacroProgramTextKind, TableSchema, read_table};
use crate::{NativeImage, header::check};
#[cfg(feature = "journal")]
use tablecraft_core::limits::Resource;
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
        with_selection(
            self,
            table,
            budget,
            cancelled,
            |coverage, selectors, budget| {
                coverage.prepare_before_change_fields(selectors, original, budget, cancelled)
            },
        )
    }
}

fn with_selection<'image, T>(
    image: &'image NativeImage,
    table: ObjectId,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    finish: impl FnOnce(
        &crate::TableMacroCoverage<'image>,
        &[BeforeEventGuardSelector<'_>],
        &mut ResourceBudget,
    ) -> Result<T, BeforeEventGuardError>,
) -> Result<T, BeforeEventGuardError> {
    let bound = binding::capture(image, table, budget, cancelled)?;
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
    finish(&bound.coverage, &selectors, budget)
}

/// Resolve every observed native reference before constructing private
/// projected compiler admission. No caller-selected list enters this route.
pub(in crate::table::mutation) fn programs<'image>(
    image: &'image NativeImage,
    schema: &crate::TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<super::fields::batch::SelectedPrograms<'image>, BeforeEventGuardError> {
    with_selection(
        image,
        schema.object_id(),
        budget,
        cancelled,
        |_, selectors, budget| {
            super::fields::batch::SelectedPrograms::capture(
                image, schema, selectors, budget, cancelled,
            )
        },
    )
}

/// Original static SetField target union, not mutation or absence authority.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn potential_targets(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<usize>, BeforeEventGuardError> {
    let snapshot = read_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if snapshot.schema().object_id() != schema.object_id()
        || snapshot.schema().definition_page() != schema.definition_page()
        || snapshot.columns().len() != schema.columns().len()
    {
        return Err(BeforeEventGuardError::Preservation);
    }
    with_selection(
        image,
        schema.object_id(),
        budget,
        cancelled,
        |coverage, selectors, budget| {
            // Preserve opaque/envelope/dormant validation before observing names.
            let admitted = SelectedPrograms::capture(image, schema, selectors, budget, cancelled)?;
            admitted
                .require_source(image, schema, budget, cancelled)
                .map_err(error::intent)?;
            let mut targets = buffer::vector(schema.columns().len(), budget, cancelled)?;
            for selector in selectors {
                let selected =
                    selection::resolve(coverage.table_extra(), *selector, budget, cancelled)?;
                let program = selected
                    .value
                    .inspect_complete_before_events(budget, cancelled)?;
                for (node, instruction) in program.nodes().iter().enumerate() {
                    check(budget, cancelled)?;
                    if !matches!(instruction.kind(), MacroProgramNodeKind::SetField { .. }) {
                        continue;
                    }
                    let name = program
                        .text_content(node, MacroProgramTextKind::Field, budget, cancelled)?
                        .ok_or(InspectError::Malformed)?;
                    let position = expression::target(
                        name.text(),
                        Fields::Complete(&snapshot),
                        budget,
                        cancelled,
                    )?;
                    budget
                        .charge(Resource::Steps, targets.len() as u64)
                        .map_err(InspectError::from)?;
                    if !targets.contains(&position) {
                        targets.push(position);
                    }
                }
            }
            Ok(targets)
        },
    )
}
