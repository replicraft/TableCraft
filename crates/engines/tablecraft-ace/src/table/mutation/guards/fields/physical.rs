//! One explicit original-program/request reconstruction and one fitting batch.
use super::super::{
    BeforeEvent, BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents,
    BeforeGuardProgramRevision, BeforeGuardRejection, BeforeGuardTrace, buffer, compile, source,
};
use super::{BeforeChangeAssignment, Prepared, prepare};
use crate::{
    Ace16WritePreview, EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage,
    TableSchema, TableSnapshot, WriteBlocker, header::check, table::mutation::preview,
};
use std::fmt;
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
    values::Value,
};
use thiserror::Error;

/// A caller-selected BeforeChange engineering candidate, never a normal plan.
/// Program discovery/required coverage/native dispatch remain unqualified.
///
/// ```compile_fail
/// fn no_normal_save(preview: tablecraft_ace::Ace16BeforeChangePreview) {
///     let _: tablecraft_ace::NativeEditPlan = preview;
/// }
/// ```
pub struct Ace16BeforeChangePreview {
    candidate: Ace16WritePreview,
    programs: Vec<BeforeGuardProgramRevision>,
    trace: Vec<BeforeGuardTrace>,
    assignments: Vec<BeforeChangeAssignment>,
}

impl Ace16BeforeChangePreview {
    /// Exact original complete image, including original macro properties.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete independently checked fitting engineering candidate.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.candidate.candidate_bytes()
    }

    /// Distinct source rows whose jointly final values differ from the original.
    pub fn changed_rows(&self) -> usize {
        self.candidate.changed_rows()
    }

    /// Exact original explicitly selected property/program witnesses.
    pub fn programs(&self) -> &[BeforeGuardProgramRevision] {
        &self.programs
    }

    /// Completed payload-free control-flow trace.
    pub fn trace(&self) -> &[BeforeGuardTrace] {
        &self.trace
    }

    /// Executed payload-free SetField locations in statement order.
    pub fn assignments(&self) -> &[BeforeChangeAssignment] {
        &self.assignments
    }
}

/// No physical candidate is returned for any rejected/failed preparation.
#[derive(Error, PartialEq, Eq)]
pub enum BeforeChangePreviewError {
    /// Exact original logical preparation or source binding failed.
    #[error(transparent)]
    Preparation(#[from] BeforeEventGuardError),
    /// Explicit RaiseError discarded every logical/physical change.
    #[error("BeforeChange rejected the physical update")]
    Rejected(BeforeGuardRejection),
    /// Existing physical/constraint/resource qualification failed.
    #[error(transparent)]
    Edit(#[from] EditError),
}

impl fmt::Debug for BeforeChangePreviewError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preparation(error) => output.debug_tuple("Preparation").field(error).finish(),
            Self::Rejected(rejection) => output
                .debug_struct("Rejected")
                .field("number", &rejection.number())
                .finish(),
            Self::Edit(error) => output.debug_tuple("Edit").field(error).finish(),
        }
    }
}

/// Sealed source-only payload admission. No public report or caller-provided
/// final row can construct this token; it never grants map/allocation authority.
pub(in crate::table::mutation) struct SourceWitness {
    source_address: usize,
    source_length: usize,
    definition: u32,
    object: ObjectId,
    programs: Vec<BeforeGuardProgramRevision>,
}

impl SourceWitness {
    pub(in crate::table::mutation) fn require(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if self.source_address != image.as_bytes().as_ptr() as usize
            || self.source_length != image.as_bytes().len()
            || self.definition != schema.definition_page()
            || self.object != schema.object_id()
        {
            return Err(EditError::Preservation);
        }
        let table = source::table(image, schema.name(), budget, cancelled).map_err(edit_error)?;
        let mut selectors = buffer::vector(self.programs.len(), budget, cancelled)?;
        for program in &self.programs {
            check(budget, cancelled)?;
            selectors.push(BeforeEventGuardSelector::new(
                program.group(),
                program.name(),
            ));
        }
        // Recompilation preserves complete Extra coverage, body/module/children
        // refusals, original property order/bytes and zero-row fail-closed rules.
        let rebound = compile::derive_fields(
            image,
            &table,
            &selectors,
            BeforeEvent::Update,
            budget,
            cancelled,
        )
        .map_err(edit_error)?;
        let mut work = self
            .programs
            .len()
            .checked_add(rebound.programs.len())
            .ok_or(InspectError::StructureLimit)?;
        for program in self.programs.iter().chain(&rebound.programs) {
            work = work
                .checked_add(program.raw.len())
                .and_then(|value| value.checked_add(program.name.len()))
                .ok_or(InspectError::StructureLimit)?;
        }
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        if self.programs != rebound.programs {
            return Err(EditError::Preservation);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

fn edit_error(error: BeforeEventGuardError) -> EditError {
    match error {
        BeforeEventGuardError::Inspect(error) => EditError::Inspect(error),
        BeforeEventGuardError::Intent(error) => error,
        BeforeEventGuardError::Preservation => EditError::Preservation,
        _ => EditError::Unsupported(WriteBlocker::ActiveContent),
    }
}

fn derive_columns(
    table: &TableSnapshot,
    prepared: &Prepared,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<FixedScalarUpdate>>, BeforeEventGuardError> {
    let rows = prepared
        .execution
        .rows
        .as_ref()
        .ok_or(BeforeEventGuardError::Preservation)?;
    if table.columns().len() > NativeImage::MAX_FIXED_BATCH_GROUPS
        || rows
            .len()
            .checked_mul(table.columns().len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .is_none()
    {
        return Err(InspectError::StructureLimit.into());
    }
    let mut groups = buffer::vector(table.columns().len(), budget, cancelled)?;
    for column in 0..table.columns().len() {
        check(budget, cancelled)?;
        let mut changes = buffer::vector(rows.len(), budget, cancelled)?;
        for final_row in rows {
            check(budget, cancelled)?;
            let before = table
                .rows()
                .get(final_row.ordinal())
                .and_then(|row| row.get(column))
                .ok_or(BeforeEventGuardError::Preservation)?;
            let after = final_row
                .values()
                .get(column)
                .ok_or(BeforeEventGuardError::Preservation)?;
            if !matches!(before, Value::Int32(_) | Value::Null)
                || !matches!(after, Value::Int32(_) | Value::Null)
            {
                return Err(BeforeEventGuardError::Preservation);
            }
            changes.push(FixedScalarUpdate::new(
                final_row.ordinal(),
                before.clone(),
                after.clone(),
            ));
        }
        groups.push(changes);
    }
    Ok(groups)
}

fn prove_groups(
    image: &NativeImage,
    table_name: &str,
    selectors: &[BeforeEventGuardSelector<'_>],
    original: BeforeEventIntents<'_>,
    observed: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    let rebound = prepare(image, table_name, selectors, original, budget, cancelled)?;
    let expected = rebound
        .execution
        .rows
        .as_ref()
        .ok_or(BeforeEventGuardError::Preservation)?;
    let table = source::table(image, table_name, budget, cancelled)?;
    if observed.len() != table.columns().len() {
        return Err(BeforeEventGuardError::Preservation);
    }
    for (column, group) in observed.iter().enumerate() {
        check(budget, cancelled)?;
        let name = table.columns()[column].name();
        budget
            .charge(
                Resource::Steps,
                group.table().len().max(table_name.len()) as u64,
            )
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, group.column().len().max(name.len()) as u64)
            .map_err(InspectError::from)?;
        if group.table() != table_name
            || group.column() != name
            || group.updates().len() != expected.len()
        {
            return Err(BeforeEventGuardError::Preservation);
        }
        for (actual, expected) in group.updates().iter().zip(expected) {
            check(budget, cancelled)?;
            let before = table
                .rows()
                .get(expected.ordinal())
                .and_then(|row| row.get(column))
                .ok_or(BeforeEventGuardError::Preservation)?;
            let after = expected
                .values()
                .get(column)
                .ok_or(BeforeEventGuardError::Preservation)?;
            if actual.row() != expected.ordinal()
                || actual.expected() != before
                || actual.value() != after
            {
                return Err(BeforeEventGuardError::Preservation);
            }
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

impl NativeImage {
    /// Prepare explicit complete BeforeChange programs and apply their jointly
    /// final ordinary Int32/NULL rows through one existing fitting batch. This
    /// distinct candidate grants no native dispatch, required-coverage or normal
    /// save permission. Original macro/catalog/map bytes remain unchanged.
    pub fn preview_ace16_before_change_updates(
        self,
        table_name: &str,
        selectors: &[BeforeEventGuardSelector<'_>],
        column: &str,
        original_updates: &[FixedScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16BeforeChangePreview, BeforeChangePreviewError> {
        let _span = tracing::debug_span!("native.before_change.preview").entered();
        preview::qualify(&self)?;
        let original = BeforeEventIntents::FixedUpdate {
            column,
            updates: original_updates,
        };
        let mut prepared = prepare(&self, table_name, selectors, original, budget, cancelled)?;
        if let Some(rejection) = prepared.execution.rejection.take() {
            return Err(BeforeChangePreviewError::Rejected(rejection));
        }
        let table = source::table(&self, table_name, budget, cancelled)?;
        let changes = derive_columns(&table, &prepared, budget, cancelled)?;
        let mut groups = buffer::vector(changes.len(), budget, cancelled)
            .map_err(BeforeEventGuardError::from)?;
        for (column, updates) in changes.iter().enumerate() {
            check(budget, cancelled).map_err(BeforeEventGuardError::from)?;
            groups.push(FixedColumnUpdates::new(
                table_name,
                table.columns()[column].name(),
                updates,
            ));
        }
        prove_groups(
            &self, table_name, selectors, original, &groups, budget, cancelled,
        )?;
        let witness = SourceWitness {
            source_address: self.as_bytes().as_ptr() as usize,
            source_length: self.as_bytes().len(),
            definition: table.schema().definition_page(),
            object: prepared.object,
            programs: prepared.compiled.programs,
        };
        let candidate = self.preview_before_change_batch(&groups, &witness, budget, cancelled)?;
        check(budget, cancelled).map_err(BeforeEventGuardError::from)?;
        Ok(Ace16BeforeChangePreview {
            candidate,
            programs: witness.programs,
            trace: prepared.execution.trace,
            assignments: prepared.execution.assignments,
        })
    }
}

#[cfg(test)]
mod tests;
