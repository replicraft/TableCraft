//! Isolated named-query parameter scopes and bounded dependency traversal.
use super::{Binder, Parameter};
use crate::{
    QueryError,
    plan::{ProfiledType, ReadPlan, Relation, RelationNode, Slot},
    profile::{MAX_NAMED_DEPTH, MAX_PARAMETERS, SOURCE_SCHEMA_POLICY},
};
use tablecraft_sql::query::{Query, Statement};

impl Binder<'_> {
    pub(super) fn named_relation(
        &mut self,
        read: ReadPlan,
        qualifier: &str,
    ) -> Result<Relation, QueryError> {
        self.charge(size_of::<ReadPlan>() + read.names.len() * size_of::<Slot>())?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(read.names.len())
            .map_err(|_| QueryError::Allocation)?;
        for (index, (((name, policy), _), declaration)) in read
            .names
            .iter()
            .zip(&read.text_policies)
            .zip(&read.output_types)
            .zip(&read.declarations)
            .enumerate()
        {
            slots.push(Slot {
                output_type: ProfiledType {
                    materialized: read.materialized_types[index],
                    declared: read.declared_types[index],
                },
                declaration: *declaration,
                text_policy: *policy,
                available: true,
                member: None,
                name: self.copy_name(name)?,
                qualifier: Some(self.copy_name(qualifier)?),
            });
        }
        Ok(Relation {
            slots,
            node: RelationNode::Derived(Box::new(read)),
        })
    }
    pub(super) fn copy_name(&mut self, name: &str) -> Result<String, QueryError> {
        self.charge(name.len())?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(name.len())
            .map_err(|_| QueryError::Allocation)?;
        owned.push_str(name);
        Ok(owned)
    }
    pub(super) fn scope_indices(&mut self, width: usize) -> Result<Vec<usize>, QueryError> {
        self.charge(width * size_of::<usize>())?;
        let mut scope = Vec::new();
        scope
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        scope.extend(0..width);
        Ok(scope)
    }
    pub(super) fn named_read(
        &mut self,
        name: &str,
        query: &Query,
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        self.tick(depth)?;
        if self
            .active_queries
            .iter()
            .any(|active| SOURCE_SCHEMA_POLICY.names.equals(active, name))
        {
            return Err(QueryError::DependencyCycle);
        }
        if self.active_queries.len() == MAX_NAMED_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        if !matches!(query.statement, Statement::Read(_) | Statement::Crosstab(_)) {
            return Err(QueryError::Unsupported);
        }
        if matches!(query.statement, Statement::Read(_)) {
            let materialized = self.materialized;
            for cached in materialized {
                self.tick(1)?;
                if SOURCE_SCHEMA_POLICY.names.equals(&cached.name, name) {
                    return Err(QueryError::SchemaChanged);
                }
            }
        }
        let declarations = self.bind_parameters(&query.parameters)?;
        let mut scope = self.scope_indices(declarations.len())?;
        for (position, parameter) in declarations.into_iter().enumerate() {
            let existing = self.parameters.iter().position(|p| {
                SOURCE_SCHEMA_POLICY
                    .names
                    .equals(p.name.as_str(), parameter.name.as_str())
            });
            scope[position] = if let Some(index) = existing {
                if self.parameters[index].data_type != parameter.data_type {
                    return Err(QueryError::Parameter);
                }
                index
            } else {
                if self.parameters.len() == MAX_PARAMETERS {
                    return Err(QueryError::StructureLimit);
                }
                self.charge(size_of::<Parameter>())?;
                self.parameters
                    .try_reserve(1)
                    .map_err(|_| QueryError::Allocation)?;
                let index = self.parameters.len();
                self.parameters.push(parameter);
                index
            };
        }
        self.charge(size_of::<String>() + name.len())?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(name.len())
            .map_err(|_| QueryError::Allocation)?;
        owned.push_str(name);
        self.active_queries
            .try_reserve(1)
            .map_err(|_| QueryError::Allocation)?;
        self.active_queries.push(owned);
        let previous_scope = std::mem::replace(&mut self.parameter_scope, scope);
        // Named sources are independent definitions, never lateral relations.
        // Each stored definition has its own bounded syntax depth. The separate
        // dependency path ceiling and global node/byte budgets bound their nesting.
        let result = match &query.statement {
            Statement::Read(read) => self.read(read, &[], 1),
            Statement::Crosstab(cross) => self.fixed_crosstab(cross, query),
            _ => Err(QueryError::Unsupported),
        };
        self.parameter_scope = previous_scope;
        self.active_queries.pop();
        result
    }
}
