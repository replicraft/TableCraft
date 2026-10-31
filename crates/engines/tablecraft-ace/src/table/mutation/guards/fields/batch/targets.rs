//! Declared user targets plus compiled writes, never readonly frame operands.
use super::super::super::{BeforeEventGuardError, buffer, compile::Operation};
use super::{
    bound::{BoundPrograms, SourceFacts},
    requests::Batch,
};
use crate::{InspectError, NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Target {
    pub native: usize,
    pub snapshot: usize,
    pub frame: Option<usize>,
    pub user: Option<usize>,
    pub macro_write: bool,
}

pub(super) struct Targets {
    pub items: Vec<Target>,
}

pub(super) fn bind(
    source: SourceFacts<'_>,
    programs: &BoundPrograms<'_, '_, '_>,
    batch: &Batch<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Targets, BeforeEventGuardError> {
    programs.projection().require_table(source.table)?;
    let capacity = batch
        .groups
        .len()
        .checked_add(programs.compiled().instructions.len())
        .ok_or(InspectError::StructureLimit)?
        .min(source.table.schema().columns().len());
    let mut items = buffer::vector(capacity, budget, cancelled)?;
    for (user, group) in batch.groups.iter().enumerate() {
        check(budget, cancelled)?;
        items.push(Target {
            native: group.native,
            snapshot: group.snapshot,
            frame: group.frame,
            user: Some(user),
            macro_write: false,
        });
    }
    for instruction in &programs.compiled().instructions {
        check(budget, cancelled)?;
        let Operation::SetField { column, .. } = &instruction.operation else {
            continue;
        };
        let projection = programs.projection();
        let native = projection.native(*column)?;
        projection.require_target(*column)?;
        let mut found = false;
        for target in &mut items {
            check(budget, cancelled)?;
            if target.native == native {
                target.macro_write = true;
                found = true;
                break;
            }
        }
        if !found {
            if items.len() >= NativeImage::MAX_FIXED_BATCH_GROUPS || items.len() >= capacity {
                return Err(InspectError::StructureLimit.into());
            }
            items.push(Target {
                native,
                snapshot: projection.snapshot_position(*column)?,
                frame: Some(*column),
                user: None,
                macro_write: true,
            });
        }
    }

    // Bound the actual final group expansion before evaluating any event. A
    // potential SetField is represented on every event row, including false
    // conditions;
    // those resulting no-op requests remain subject to metadata admission.
    let mut count = 0_usize;
    for target in &items {
        check(budget, cancelled)?;
        let width = if target.macro_write {
            batch.ordinals.len()
        } else {
            batch
                .groups
                .get(target.user.ok_or(InspectError::Malformed)?)
                .ok_or(InspectError::Malformed)?
                .updates
                .len()
        };
        count = count
            .checked_add(width)
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
    }
    check(budget, cancelled)?;
    Ok(Targets { items })
}
