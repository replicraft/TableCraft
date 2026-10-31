//! Complete field binding after the exact Extra has been source-admitted.
use super::super::super::{BeforeEventGuardError, BeforeGuardProgramRevision, compile, error};
use super::{admission::SelectedPrograms, projection::Int32Projection};
use crate::{IndexInspection, NativeImage, TableSnapshot, header::check};
use tablecraft_core::limits::ResourceBudget;

/// Borrowed actual facts from the one original preserved Source. No stage token,
/// reconstructed bytes or projected fake table is accepted here.
#[derive(Clone, Copy)]
pub(in crate::table::mutation) struct SourceFacts<'source> {
    pub table: &'source TableSnapshot,
    pub indexes: &'source IndexInspection,
    pub dependencies: &'source [usize],
}

/// This owner is unavailable until every selected/dormant definition has been
/// field-bound and every potential SetField target has passed metadata admission.
/// It still grants neither required-event discovery nor ordinary-save authority.
pub(in crate::table::mutation) struct BoundPrograms<'selected, 'image, 'source> {
    selected: &'selected SelectedPrograms<'image>,
    projection: Int32Projection<'source>,
    compiled: compile::Compiled,
}

impl<'selected, 'image, 'source> BoundPrograms<'selected, 'image, 'source> {
    pub(in crate::table::mutation) fn bind(
        image: &NativeImage,
        selected: &'selected SelectedPrograms<'image>,
        source: SourceFacts<'source>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, BeforeEventGuardError> {
        selected
            .require_source(image, source.table.schema(), budget, cancelled)
            .map_err(error::intent)?;
        let projection = Int32Projection::capture(
            source.table,
            source.indexes,
            source.dependencies,
            budget,
            cancelled,
        )?;
        let selectors = selected.selectors(budget, cancelled)?;
        let compiled =
            compile::derive_projected(image, &projection, &selectors, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            selected,
            projection,
            compiled,
        })
    }

    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        source: SourceFacts<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), BeforeEventGuardError> {
        self.projection.require_table(source.table)?;
        self.selected
            .require_source(image, source.table.schema(), budget, cancelled)
            .map_err(error::intent)
    }

    pub(in crate::table::mutation::guards) fn compiled(&self) -> &compile::Compiled {
        &self.compiled
    }

    pub(in crate::table::mutation) fn projection(&self) -> &Int32Projection<'source> {
        &self.projection
    }

    pub(super) fn into_programs(self) -> Vec<BeforeGuardProgramRevision> {
        self.compiled.programs
    }
}
