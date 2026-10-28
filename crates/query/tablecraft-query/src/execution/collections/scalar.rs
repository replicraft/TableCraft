//! Scalar totals consume completed derived lists without comparing their values.
use super::{CollectionFrame, Runtime};
use crate::{
    QueryError,
    plan::{Expr, ReadPlan, RelationNode, SelectPlan},
    profile::MAX_RUNTIME_READ_DEPTH,
};

impl Runtime<'_, '_> {
    pub(super) fn check_scalar_collection_aggregate(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if select.parent.is_some()
            || !matches!(&select.source.node, RelationNode::Derived { expansion, .. } if expansion.is_empty())
        {
            return Err(QueryError::Unsupported);
        }
        for output in &plan.output_types {
            self.tick()?;
            if output.is_multivalue() {
                return Err(QueryError::Unsupported);
            }
        }

        for expression in select
            .projection
            .iter()
            .chain(select.filter.iter())
            .chain(&select.group)
            .chain(select.having.iter())
        {
            self.check_collection_scalar_expression(expression, 0)?;
        }
        for (expression, _) in &plan.sort {
            self.check_collection_scalar_expression(expression, 0)?;
        }
        Ok(())
    }

    fn check_collection_scalar_expression(
        &mut self,
        expression: &Expr,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if depth > MAX_RUNTIME_READ_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        match expression {
            Expr::Literal(_)
            | Expr::NumericLiteral(_)
            | Expr::DateLiteral(..)
            | Expr::Parameter(_)
            | Expr::Output(..)
            | Expr::Projected(_)
            | Expr::Field { depth: 0, .. } => Ok(()),
            Expr::Unary(_, value) | Expr::IsNull(value, _) => {
                self.check_collection_scalar_expression(value, depth + 1)
            }
            Expr::Binary(_, left, right) => {
                self.check_collection_scalar_expression(left, depth + 1)?;
                self.check_collection_scalar_expression(right, depth + 1)
            }
            Expr::Aggregate(_, argument) => {
                if let Some(argument) = argument {
                    self.check_collection_scalar_expression(argument, depth + 1)?;
                }
                Ok(())
            }
            // Membership/correlated/domain contexts need their original frames.
            // Unreviewed scalar call shapes stay outside this initial boundary.
            _ => Err(QueryError::Unsupported),
        }
    }

    pub(super) fn scalar_collection_aggregate(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
        input: Vec<CollectionFrame>,
    ) -> Result<Vec<CollectionFrame>, QueryError> {
        self.check_scalar_collection_aggregate(select, plan)?;
        let mut scalars = Vec::new();
        for frame in input {
            self.tick()?;
            // collection_input completed every selected inner list/projection.
            // Moving these cells neither prunes that work nor refunds its cost.
            self.push(&mut scalars, frame.scalars)?;
        }
        let projected = self.finish_select(scalars, select, plan, &[])?;
        let finished = self.finish_read(projected, plan)?;
        let mut result = Vec::new();
        for scalars in finished {
            self.push(
                &mut result,
                CollectionFrame {
                    scalars,
                    lists: Vec::new(),
                    keys: Vec::new(),
                },
            )?;
        }
        Ok(result)
    }
}
