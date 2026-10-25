//! Prior explicit projection aliases have one lexical SELECT owner.
mod stability;

use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ProjectionReference, Slot},
    profile::SOURCE_SCHEMA_POLICY,
};

#[derive(Default)]
pub(super) struct ProjectionAliases {
    pub enabled: bool,
    pub used: bool,
    values: Vec<Alias>,
}

struct Alias {
    name: String,
    reference: ProjectionReference,
    stable: bool,
}

impl Binder<'_> {
    pub(super) fn register_projection_alias(
        &mut self,
        name: &str,
        index: usize,
        expression: &Expr,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<(), QueryError> {
        let reference = ProjectionReference {
            index,
            output_type: self.profiled_result_type(expression, scopes, depth + 1)?,
            declaration: self.result_declaration(expression, scopes, depth + 1)?,
            text_policy: self.text_policy(expression)?,
        };
        let stable = self.stable_alias_expression(expression, depth + 1)?;
        let name = self.copy_name(name)?;
        self.charge(size_of::<Alias>())?;
        self.aliases
            .values
            .try_reserve(1)
            .map_err(|_| QueryError::Allocation)?;
        self.aliases.values.push(Alias {
            name,
            reference,
            stable,
        });
        Ok(())
    }

    pub(super) fn projection_alias(
        &mut self,
        name: &str,
        depth: usize,
    ) -> Result<Option<Expr>, QueryError> {
        if !self.aliases.enabled {
            return Ok(None);
        }
        let mut matched = None;
        for index in 0..self.aliases.values.len() {
            self.tick(depth)?;
            let alias = &self.aliases.values[index];
            if SOURCE_SCHEMA_POLICY.names.equals(&alias.name, name) {
                if matched.is_some() {
                    return Err(QueryError::AmbiguousName);
                }
                matched = Some(index);
            }
        }
        let Some(index) = matched else {
            return Ok(None);
        };
        let alias = &self.aliases.values[index];
        if !alias.stable {
            return Err(QueryError::Unsupported);
        }
        let reference = alias.reference;
        self.aliases.used = true;
        Ok(Some(Expr::Projected(reference)))
    }
}
