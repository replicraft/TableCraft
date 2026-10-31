//! Borrowed private field resolution, retaining the old complete-table loop.
use super::{
    BeforeEventGuardError, BeforeGuardUnsupported, fields::batch::projection::Int32Projection,
};
use crate::{InspectError, TableSnapshot, header::check};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
};

#[derive(Clone, Copy)]
pub(super) enum Fields<'a> {
    Complete(&'a TableSnapshot),
    Projected(&'a Int32Projection<'a>),
}

impl Fields<'_> {
    pub(super) fn object_id(self) -> ObjectId {
        match self {
            Self::Complete(table) => table.schema().object_id(),
            Self::Projected(projection) => projection.object_id(),
        }
    }

    pub(super) fn target(
        self,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, BeforeEventGuardError> {
        let position = self.position(name, budget, cancelled)?;
        if let Self::Projected(projection) = self {
            projection.require_target(position)?;
        }
        Ok(position)
    }

    pub(super) fn position(
        self,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, BeforeEventGuardError> {
        match self {
            Self::Complete(table) => {
                // Same iteration, charges, comparison and polls as old position().
                for (position, column) in table.columns().iter().enumerate() {
                    check(budget, cancelled)?;
                    budget
                        .charge(Resource::Steps, (column.name().len() + name.len()) as u64)
                        .map_err(InspectError::from)?;
                    if column.name().eq_ignore_ascii_case(name) {
                        return Ok(position);
                    }
                }
            }
            Self::Projected(projection) => {
                for position in 0..projection.len() {
                    check(budget, cancelled)?;
                    let column = projection.name(position)?;
                    budget
                        .charge(Resource::Steps, (column.len() + name.len()) as u64)
                        .map_err(InspectError::from)?;
                    if column.eq_ignore_ascii_case(name) {
                        return Ok(position);
                    }
                }
            }
        }
        Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::Context,
        ))
    }
}
