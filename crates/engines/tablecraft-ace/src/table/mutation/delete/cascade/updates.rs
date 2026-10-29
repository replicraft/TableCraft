//! Bounded NULL closure over original UPDATE associations and final tuple proof.
mod binding;
pub(super) mod facts;

use super::{
    EditError, reserve,
    state::{State, Table},
};
use crate::{InspectError, NativeImage, NativeRelationship, WriteBlocker, header::check};
use binding::Link;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ReferentialAction;

pub(super) fn close(
    state: &mut State,
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // No new projection, graph or table read on the existing no-NULL path.
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, state.tables.len() as u64)
        .map_err(InspectError::from)?;
    if state.tables.iter().all(|table| table.nulls.is_empty()) {
        return Ok(());
    }
    let relationships = std::mem::take(&mut state.relationships);
    let mut links = Vec::new();
    loop {
        discover(state, image, &relationships, &mut links, budget, cancelled)?;
        let mut changed = false;
        for link in &links {
            changed |= link.propagate(state, false, budget, cancelled)?;
        }
        if !changed {
            break;
        }
    }
    for link in &links {
        link.propagate(state, true, budget, cancelled)?;
    }
    state.relationships = relationships;
    Ok(())
}

fn discover(
    state: &mut State,
    image: &NativeImage,
    relationships: &[NativeRelationship],
    links: &mut Vec<Link>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut parent = 0;
    while parent < state.tables.len() {
        check(budget, cancelled)?;
        if state.tables[parent].nulls.is_empty() {
            parent += 1;
            continue;
        }
        for (declaration, relationship) in relationships.iter().enumerate() {
            check(budget, cancelled)?;
            let rule = relationship.declaration();
            if !rule
                .parent_table()
                .as_str()
                .eq_ignore_ascii_case(state.tables[parent].before.schema().name())
            {
                continue;
            }
            budget
                .charge(Resource::Steps, links.len() as u64)
                .map_err(InspectError::from)?;
            if links
                .iter()
                .any(|link| link.parent == parent && link.declaration == declaration)
                || !touched(&state.tables[parent], relationship, budget, cancelled)?
            {
                continue;
            }
            let rules = rule
                .rules()
                .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
            if !rules.enforced
                || !matches!(
                    rules.on_update,
                    ReferentialAction::Cascade | ReferentialAction::Restrict
                )
            {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            let child = state.load(
                image,
                rule.child_table().as_str(),
                relationships,
                budget,
                cancelled,
            )?;
            facts::capture(state, parent, image, relationships, budget, cancelled)?;
            facts::capture(state, child, image, relationships, budget, cancelled)?;
            let link = binding::bind(
                state,
                parent,
                child,
                declaration,
                relationship,
                budget,
                cancelled,
            )?;
            reserve(links, budget)?;
            links.push(link);
        }
        parent += 1;
    }
    Ok(())
}

fn touched(
    table: &Table,
    relationship: &NativeRelationship,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    for &(_, column) in &table.nulls {
        check(budget, cancelled)?;
        let native = table
            .before
            .schema()
            .columns()
            .get(column)
            .ok_or(InspectError::Malformed)?;
        for pair in relationship.declaration().columns() {
            check(budget, cancelled)?;
            if pair.parent().as_str().eq_ignore_ascii_case(native.name()) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

impl Link {
    fn propagate(
        &self,
        state: &mut State,
        final_check: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let mut changed = false;
        for &(parent, child) in &self.matches {
            check(budget, cancelled)?;
            if state.tables[self.parent].selected[parent]
                || state.tables[self.child].selected[child]
            {
                continue;
            }
            let mut activated = false;
            for &column in &self.parents[..self.width] {
                check(budget, cancelled)?;
                activated |= state.tables[self.parent].is_null(parent, column, budget)?;
            }
            if !activated {
                continue;
            }
            if self.action == ReferentialAction::Restrict {
                return Err(EditError::ReferentialAction);
            }
            for component in 0..self.width {
                check(budget, cancelled)?;
                let source = self.parents[component];
                let target = self.children[component];
                if final_check {
                    if state.tables[self.parent].value(parent, source, budget)?
                        != state.tables[self.child].value(child, target, budget)?
                    {
                        return Err(EditError::Conflict);
                    }
                } else if state.tables[self.parent].is_null(parent, source, budget)? {
                    changed |= state.assign_null(self.child, child, target, budget, cancelled)?;
                }
            }
        }
        Ok(changed)
    }
}
