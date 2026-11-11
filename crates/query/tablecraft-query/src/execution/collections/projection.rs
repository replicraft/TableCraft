//! Scalar evaluation and complete list copying retain written projection order.
use super::{CollectionFrame, Runtime};
use crate::{
    MultivalueValue, QueryError,
    execution::Environment,
    plan::{Expr, ReadPlan, SelectPlan},
};
use std::sync::Arc;
use tablecraft_core::{limits::Resource, values::Value};

impl Runtime<'_, '_> {
    pub(super) fn collection_project(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
        frame: &CollectionFrame,
    ) -> Result<Option<CollectionFrame>, QueryError> {
        let env = Environment {
            aggregate_results: None,
            collections: None,
            row: &frame.scalars,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &select.filter
            && !self.eval_predicate(filter, env)?.is_true()
        {
            return Ok(None);
        }
        let mut scalars = self.allocate_expression_row(
            select
                .projection
                .iter()
                .filter(|value| !matches!(value, Expr::Multivalue { .. } | Expr::Recordset { .. }))
                .count(),
        )?;
        let mut lists = Vec::new();
        let mut recordsets = Vec::new();
        for value in &select.projection {
            self.tick()?;
            match value {
                Expr::Multivalue { index, .. } => {
                    let mut found = None;
                    for (slot, list) in &frame.lists {
                        self.tick()?;
                        if *slot == *index {
                            found = Some(list);
                            break;
                        }
                    }
                    let list = found.ok_or(QueryError::Source)?;
                    let copied = self.copy_collection_presence(list.as_ref())?;
                    let output_slot = lists.len();
                    self.push(&mut lists, (output_slot, copied))?;
                }
                Expr::Recordset { origin, .. } => {
                    let mut found = None;
                    for (actual, value) in &frame.recordsets {
                        self.tick()?;
                        if actual == origin {
                            found = Some(value);
                            break;
                        }
                    }
                    let value = found.ok_or(QueryError::Source)?;
                    // NO_CLONE: Aliases retain one immutable parent-associated
                    // snapshot; projection does not copy its child rows.
                    let value = value.as_ref().map(Arc::clone);
                    self.push(&mut recordsets, (*origin, value))?;
                }
                value => scalars.push(self.retained_expression_operand(
                    value,
                    Environment {
                        output: &scalars,
                        ..env
                    },
                )?),
            }
        }
        let mut keys = self.allocate_expression_row(plan.sort.len())?;
        for (key, _) in &plan.sort {
            keys.push(self.comparison_key(
                key,
                Environment {
                    output: &scalars,
                    ..env
                },
            )?);
        }
        Ok(Some(CollectionFrame {
            scalars,
            lists,
            recordsets,
            keys,
        }))
    }

    pub(super) fn copy_collection_presence(
        &mut self,
        value: Option<&MultivalueValue>,
    ) -> Result<Option<MultivalueValue>, QueryError> {
        match value {
            Some(value) => self.copy_collection_values(value.values()).map(Some),
            None => {
                self.tick()?;
                Ok(None)
            }
        }
    }

    // A stored list is data, not a scalar arithmetic operand. Preserve every
    // IEEE representation while retaining ordinary payload bounds for other kinds.
    pub(super) fn copy_stored_list_value(&mut self, value: &Value) -> Result<Value, QueryError> {
        match value {
            Value::Single(_) | Value::Double(_) => {
                self.tick()?;
                // NO_CLONE: Return fixed-width stored IEEE bits unchanged, including nonfinite
                // representations, for collection storage; generic scalar copy intentionally
                // rejects nonfinite arithmetic operands.
                Ok(value.clone())
            }
            value => self.copy(value),
        }
    }

    pub(super) fn copy_collection_values(
        &mut self,
        values: &[Value],
    ) -> Result<MultivalueValue, QueryError> {
        self.tick()?;
        self.budget.charge(Resource::Rows, values.len() as u64)?;
        self.budget
            .charge_allocation(values.len() as u64, size_of::<Value>() as u64)?;
        let mut copied = Vec::new();
        copied
            .try_reserve_exact(values.len())
            .map_err(|_| QueryError::Allocation)?;
        for value in values {
            copied.push(self.copy_stored_list_value(value)?);
        }
        Ok(MultivalueValue { values: copied })
    }
}
