//! Stateful continuations advance only after successful upstream logical EOF.
mod driver;
mod state;

use super::{
    aggregates::GlobalInput, groups::GroupInput, having::HavingInput, retained::StageOutput,
    text_groups::Captured,
};
use crate::plan::{Expr, ReadPlan, SelectPlan};

struct Specification<'p> {
    read: &'p ReadPlan,
    select: &'p SelectPlan,
    aggregates: Vec<&'p Expr>,
    input_width: usize,
}

impl<'p> Specification<'p> {
    fn output(&self) -> StageOutput<'p, '_> {
        StageOutput {
            read: self.read,
            select: self.select,
            aggregates: &self.aggregates,
        }
    }
}

struct Stage<'p> {
    specification: Specification<'p>,
    state: Option<InputState<'p>>,
}

enum InputState<'p> {
    Row(u64),
    Global(GlobalInput),
    Exact(GroupInput),
    Having(HavingInput<'p>),
    Text(Captured<'p>),
    Finished,
}
