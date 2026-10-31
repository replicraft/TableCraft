//! Read admission for exact explicitly selected programs, not write authority.
use super::super::super::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeGuardUnsupported, buffer, selection,
};
use crate::{
    EditError, InspectError, MacroProgramNodeKind, NativeImage, PayloadPresence,
    TableExtraCoverage, TableMacroCoverage, TableSchema, header::check,
};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
};

const BEFORE_CHANGE_FLAGS: u8 = 0x01;
const COMPARE_CHUNK: usize = 4096;

struct Selector {
    group: u16,
    name: String,
}

/// A private selected-program payload observation. Constructed from the exact
/// immutable source; it only permits that Source reader to inspect this Extra.
/// A separate complete field binding/target proof is mandatory before staging.
pub(in crate::table::mutation) struct SelectedPrograms<'image> {
    coverage: TableMacroCoverage<'image>,
    object: ObjectId,
    definition: u32,
    selectors: Vec<Selector>,
}

impl<'image> SelectedPrograms<'image> {
    pub(in crate::table::mutation) fn capture(
        image: &'image NativeImage,
        schema: &TableSchema,
        selectors: &[BeforeEventGuardSelector<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, BeforeEventGuardError> {
        image.require_standard_capture()?;
        selection::validate(selectors, budget, cancelled)?;
        let coverage = image.inspect_table_macro_coverage(schema.object_id(), budget, cancelled)?;
        if coverage.body().presence() != PayloadPresence::Absent
            || coverage.module().presence() != PayloadPresence::Absent
            || !coverage.children().is_empty()
        {
            return Err(unsupported());
        }

        let extra = coverage.table_extra();
        if extra.object_id() != schema.object_id()
            || extra.presence() != PayloadPresence::Present
            || extra.coverage() != TableExtraCoverage::WideMr2
        {
            return Err(unsupported());
        }
        selection::require_complete(extra, selectors, budget, cancelled)?;
        let mut owned = buffer::vector(selectors.len(), budget, cancelled)?;
        for selector in selectors {
            check(budget, cancelled)?;
            let selected = selection::resolve(extra, *selector, budget, cancelled)?;
            if selected.value.native_type() != crate::format::data_type::LONG_TEXT
                || selected.value.flags() != BEFORE_CHANGE_FLAGS
            {
                return Err(unsupported());
            }
            let program = selected
                .value
                .inspect_complete_before_events(budget, cancelled)?;
            for (definition, envelope) in program.envelopes().definitions().iter().enumerate() {
                check(budget, cancelled)?;
                let event = envelope.event().ok_or_else(unsupported)?;
                if !matches!(event, "BeforeChange" | "BeforeDelete") {
                    return Err(unsupported());
                }
                for node in program.nodes() {
                    check(budget, cancelled)?;
                    if node.definition() != definition {
                        continue;
                    }
                    if matches!(node.kind(), MacroProgramNodeKind::Opaque { .. })
                        || (event == "BeforeDelete"
                            && matches!(node.kind(), MacroProgramNodeKind::SetField { .. }))
                    {
                        return Err(unsupported());
                    }
                }
            }

            let mut name = buffer::vector(selector.name().len(), budget, cancelled)?;
            name.extend_from_slice(selector.name().as_bytes());
            owned.push(Selector {
                group: selector.group(),
                name: String::from_utf8(name).map_err(|_| InspectError::Malformed)?,
            });
        }
        check(budget, cancelled)?;
        Ok(Self {
            coverage,
            object: schema.object_id(),
            definition: schema.definition_page(),
            selectors: owned,
        })
    }

    /// The only replacement for preserved Source's inert payload check.
    /// Image identity includes the exact captured scope/catalog, not equal bytes.
    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if !std::ptr::eq(self.coverage.image(), image)
            || self.object != schema.object_id()
            || self.definition != schema.definition_page()
        {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(super) fn selectors(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<BeforeEventGuardSelector<'_>>, BeforeEventGuardError> {
        let mut result = buffer::vector(self.selectors.len(), budget, cancelled)?;
        for selector in &self.selectors {
            check(budget, cancelled)?;
            result.push(BeforeEventGuardSelector::new(
                selector.group,
                &selector.name,
            ));
        }
        Ok(result)
    }

    /// Fresh candidate-bound admission; never retarget an original pointer token.
    /// Full candidate Source certification and independent replay are still needed.
    pub(in crate::table::mutation) fn rebind_candidate<'candidate>(
        &self,
        candidate: &'candidate NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SelectedPrograms<'candidate>, BeforeEventGuardError> {
        if self.object != schema.object_id() || self.definition != schema.definition_page() {
            return Err(BeforeEventGuardError::Preservation);
        }
        let selectors = self.selectors(budget, cancelled)?;
        let rebound = SelectedPrograms::capture(candidate, schema, &selectors, budget, cancelled)?;
        let before = self.coverage.table_extra();
        let after = rebound.coverage.table_extra();
        if before.native_id() != after.native_id()
            || before.parent_id() != after.parent_id()
            || before.catalog_page() != after.catalog_page()
            || before.catalog_slot() != after.catalog_slot()
        {
            return Err(BeforeEventGuardError::Preservation);
        }
        same_bytes(
            before.raw().ok_or(InspectError::Malformed)?,
            after.raw().ok_or(InspectError::Malformed)?,
            budget,
            cancelled,
        )?;
        Ok(rebound)
    }
}

fn unsupported() -> BeforeEventGuardError {
    BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::Structure)
}

fn same_bytes(
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    if before.len() != after.len() {
        return Err(BeforeEventGuardError::Preservation);
    }
    for (left, right) in before
        .chunks(COMPARE_CHUNK)
        .zip(after.chunks(COMPARE_CHUNK))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, left.len() as u64)
            .map_err(InspectError::from)?;
        if left != right {
            return Err(BeforeEventGuardError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
