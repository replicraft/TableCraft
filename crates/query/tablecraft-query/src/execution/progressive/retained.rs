//! Bound output references never authorize conversion of intermediate cells.
use super::StreamError;
use crate::{
    execution::cell::CellValue,
    plan::{Expr, ReadPlan, SelectPlan},
};

#[derive(Clone, Copy)]
pub(super) struct StageOutput<'p, 'r> {
    pub(super) read: &'p ReadPlan,
    pub(super) select: &'p SelectPlan,
    pub(super) aggregates: &'r [&'p Expr],
}

/// A borrowed callback owns no queue and preserves the original typed sink error.
pub(super) type RetainedReceiver<'a, R, E> =
    dyn FnMut(&mut R, Vec<CellValue>) -> Result<(), StreamError<E>> + 'a;

#[cfg(test)]
mod tests;
