//! Agreement between reciprocal logical indexes and the relationship catalog.
use super::{NativeIndex, reference::PRIMARY_TABLE};
use crate::{InspectError, NativeRelationship, TableSchema, header::check, inspect_relationships};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

pub(super) struct Relationships {
    declarations: Vec<NativeRelationship>,
    matched: Vec<usize>,
}
impl Relationships {
    pub fn read<R: Read + Seek>(
        source: &mut R,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let declarations = inspect_relationships(source, scope, budget, cancelled)?;
        budget.charge_allocation(declarations.len() as u64, size_of::<usize>() as u64)?;
        let mut matched = Vec::new();
        matched
            .try_reserve_exact(declarations.len())
            .map_err(|_| InspectError::Allocation)?;
        matched.resize(declarations.len(), 0);
        Ok(Self {
            declarations,
            matched,
        })
    }

    pub fn link(
        &mut self,
        local: (&TableSchema, &NativeIndex, &[NativeIndex]),
        other: (&TableSchema, &NativeIndex, &[NativeIndex]),
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        let reference = local
            .1
            .foreign_reference()
            .ok_or(InspectError::Unsupported)?;
        let (parent, child) = if reference.native_role() == PRIMARY_TABLE {
            (local, other)
        } else {
            (other, local)
        };
        let mut matched = None;
        for (position, native) in self.declarations.iter().enumerate() {
            check(budget, cancelled)?;
            let declaration = native.declaration();
            if !declaration
                .parent_table()
                .as_str()
                .eq_ignore_ascii_case(parent.0.name())
                || !declaration
                    .child_table()
                    .as_str()
                    .eq_ignore_ascii_case(child.0.name())
                || declaration.columns().len() != parent.1.columns().len()
            {
                continue;
            }
            let mut columns_match = true;
            for (parent_key, child_key) in parent.1.columns().iter().zip(child.1.columns()) {
                check(budget, cancelled)?;
                budget.charge(
                    Resource::Steps,
                    (parent.0.columns().len() + child.0.columns().len()) as u64,
                )?;
                let parent_name = column(parent.0, parent_key.number())?;
                let child_name = column(child.0, child_key.number())?;
                let mut pair_matches = false;
                for pair in declaration.columns() {
                    check(budget, cancelled)?;
                    pair_matches |= parent_name.eq_ignore_ascii_case(pair.parent().as_str())
                        && child_name.eq_ignore_ascii_case(pair.child().as_str());
                }
                columns_match &= pair_matches;
            }
            if !columns_match {
                continue;
            }
            let rules = declaration.rules().ok_or(InspectError::Unsupported)?;
            if !rules.enforced {
                continue;
            }
            if reference.on_update() != Some(rules.on_update)
                || reference.on_delete() != Some(rules.on_delete)
            {
                return Err(InspectError::Malformed);
            }
            let parent_constraints = parent
                .1
                .physical_constraints()
                .ok_or(InspectError::Unsupported)?;
            let child_constraints = child
                .1
                .physical_constraints()
                .ok_or(InspectError::Unsupported)?;
            // The enforcing unique key may have different sort directions and
            // therefore separate physical storage from the foreign index.
            if !parent_constraints.unique()
                || (rules.one_to_one
                    && !child_constraints.unique()
                    && !unique_key(child.1, child.2, budget, cancelled)?)
            {
                return Err(InspectError::Malformed);
            }
            if matched.replace(position).is_some() {
                return Err(InspectError::Malformed);
            }
        }
        let position = matched.ok_or(InspectError::Malformed)?;
        self.matched[position] += 1;
        Ok(())
    }

    pub fn finish(
        self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        for (relationship, count) in self.declarations.iter().zip(self.matched) {
            check(budget, cancelled)?;
            let declaration = relationship.declaration();
            let endpoints = usize::from(
                declaration
                    .parent_table()
                    .as_str()
                    .eq_ignore_ascii_case(table),
            ) + usize::from(
                declaration
                    .child_table()
                    .as_str()
                    .eq_ignore_ascii_case(table),
            );
            if endpoints == 0 {
                continue;
            }
            let rules = declaration.rules().ok_or(InspectError::Unsupported)?;
            if rules.enforced && count != endpoints {
                return Err(InspectError::Malformed);
            }
        }
        Ok(())
    }
}

fn column(schema: &TableSchema, number: usize) -> Result<&str, InspectError> {
    schema
        .columns()
        .iter()
        .find(|c| c.number() == number)
        .map(|c| c.name())
        .ok_or(InspectError::Malformed)
}

fn unique_key(
    key: &NativeIndex,
    indexes: &[NativeIndex],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    for index in indexes {
        check(budget, cancelled)?;
        if index.columns().len() != key.columns().len()
            || !index
                .physical_constraints()
                .is_some_and(|flags| flags.unique())
        {
            continue;
        }
        let mut same = true;
        for column in key.columns() {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, index.columns().len() as u64)?;
            same &= index
                .columns()
                .iter()
                .any(|other| other.number() == column.number());
        }
        if same {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
