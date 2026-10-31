//! Owned guarded-batch preparation, without exposing preserved Source.
use super::super::super::{
    BeforeEventGuardError, BeforeGuardProgramRevision, BeforeGuardRejection, BeforeGuardTrace,
    buffer,
};
use super::super::{BeforeChangeAssignment, evaluate};
use super::{
    admission::SelectedPrograms,
    bound::{BoundPrograms, SourceFacts},
    derive::{self, FinalGroup},
    frames::{self, ProjectedRow},
    requests, targets,
};
use crate::{FixedColumnUpdates, InspectError, NativeImage, TableSchema, header::check};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

pub(in crate::table::mutation) struct Prepared<'image> {
    selected: SelectedPrograms<'image>,
    object: ObjectId,
    definition: u32,
    programs: Vec<BeforeGuardProgramRevision>,
    trace: Vec<BeforeGuardTrace>,
    assignments: Vec<BeforeChangeAssignment>,
    rows: Option<Vec<ProjectedRow>>,
    rejection: Option<BeforeGuardRejection>,
    groups: Option<Vec<FinalGroup>>,
}

/// Only physical/report facts escape the preparation's source borrow.
pub(in crate::table::mutation) struct Report {
    pub programs: Vec<BeforeGuardProgramRevision>,
    pub trace: Vec<BeforeGuardTrace>,
    pub assignments: Vec<BeforeChangeAssignment>,
}

pub(in crate::table::mutation) fn prepare<'image>(
    image: &'image NativeImage,
    source: SourceFacts<'_>,
    selected: SelectedPrograms<'image>,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared<'image>, BeforeEventGuardError> {
    let bound = BoundPrograms::bind(image, &selected, source, budget, cancelled)?;
    let requests = requests::bind(source, bound.projection(), original, budget, cancelled)?;
    let targets = targets::bind(source, &bound, &requests, budget, cancelled)?;
    let frames = frames::setup(&bound, &requests, budget, cancelled)?;
    bound.require_source(image, source, budget, cancelled)?;
    let mut execution = evaluate::run_projected(bound.compiled(), &frames, budget, cancelled)?;
    let groups = match &execution.rows {
        Some(rows) if execution.rejection.is_none() => Some(derive::final_groups(
            source, &requests, &targets, rows, budget, cancelled,
        )?),
        None if execution.rejection.is_some() => None,
        _ => return Err(BeforeEventGuardError::Preservation),
    };
    derive::native_assignment_positions(&bound, &mut execution.assignments, budget, cancelled)?;
    let programs = bound.into_programs();
    check(budget, cancelled)?;
    Ok(Prepared {
        selected,
        object: source.table.schema().object_id(),
        definition: source.table.schema().definition_page(),
        programs,
        trace: execution.trace,
        assignments: execution.assignments,
        rows: execution.rows,
        rejection: execution.rejection,
        groups,
    })
}

impl Prepared<'_> {
    pub(in crate::table::mutation) fn admission(&self) -> &SelectedPrograms<'_> {
        &self.selected
    }

    pub(in crate::table::mutation) fn programs(&self) -> &[BeforeGuardProgramRevision] {
        &self.programs
    }

    pub(in crate::table::mutation) fn trace(&self) -> &[BeforeGuardTrace] {
        &self.trace
    }

    pub(in crate::table::mutation) fn assignments(&self) -> &[BeforeChangeAssignment] {
        &self.assignments
    }

    pub(in crate::table::mutation) fn rows(&self) -> Option<&[ProjectedRow]> {
        self.rows.as_deref()
    }

    pub(in crate::table::mutation) fn rejection(&self) -> Option<&BeforeGuardRejection> {
        self.rejection.as_ref()
    }

    pub(in crate::table::mutation) fn final_groups(&self) -> Option<&[FinalGroup]> {
        self.groups.as_deref()
    }

    pub(in crate::table::mutation) fn groups<'view>(
        &'view self,
        schema: &'view TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<FixedColumnUpdates<'view>>, BeforeEventGuardError> {
        if self.object != schema.object_id() || self.definition != schema.definition_page() {
            return Err(BeforeEventGuardError::Preservation);
        }
        let groups = self
            .groups
            .as_ref()
            .ok_or(BeforeEventGuardError::Preservation)?;
        let mut result = buffer::vector(groups.len(), budget, cancelled)?;
        for group in groups {
            check(budget, cancelled)?;
            let field = schema
                .columns()
                .get(group.column())
                .ok_or(InspectError::Malformed)?;
            result.push(FixedColumnUpdates::new(
                schema.name(),
                field.name(),
                group.updates(),
            ));
        }
        Ok(result)
    }

    pub(in crate::table::mutation) fn take_rejection(&mut self) -> Option<BeforeGuardRejection> {
        self.rejection.take()
    }

    /// Drops all projected values/admission before the original image is moved
    /// into its engineering result. Call only after independent audit succeeds.
    pub(in crate::table::mutation) fn into_report(self) -> Report {
        Report {
            programs: self.programs,
            trace: self.trace,
            assignments: self.assignments,
        }
    }
}
