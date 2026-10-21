use super::Pruner;
use crate::{
    QueryError,
    plan::{Expr, ReadNode, ReadPlan, Relation, RelationNode},
};

impl Pruner<'_, '_> {
    pub(super) fn collect_expr(
        &mut self,
        expr: &Expr,
        target: usize,
        offset: usize,
        needed: &mut [bool],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match expr {
            Expr::Field { depth, index } if *depth == target => {
                *needed
                    .get_mut(
                        offset
                            .checked_add(*index)
                            .ok_or(QueryError::StructureLimit)?,
                    )
                    .ok_or(QueryError::Source)? = true;
            }
            Expr::Unary(_, expr)
            | Expr::IsNull(expr, _)
            | Expr::DateCall(_, expr)
            | Expr::Convert(_, expr)
            | Expr::Inspect(_, expr)
            | Expr::DateConvert(_, expr) => self.collect_expr(expr, target, offset, needed)?,
            Expr::Binary(_, left, right) | Expr::Like(left, right, _) => {
                self.collect_expr(left, target, offset, needed)?;
                self.collect_expr(right, target, offset, needed)?;
            }
            Expr::Between(value, a, b, _) | Expr::IIf(value, a, b) | Expr::DateAdd(value, a, b) => {
                for expr in [value, a, b] {
                    self.collect_expr(expr, target, offset, needed)?;
                }
            }
            Expr::DateDiff(a, b, c, options) => {
                for expr in [a, b, c].into_iter().chain(options.iter().flatten()) {
                    self.collect_expr(expr, target, offset, needed)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for expr in [a, b].into_iter().chain(options.iter().flatten()) {
                    self.collect_expr(expr, target, offset, needed)?;
                }
            }
            Expr::TextCall(_, arguments)
            | Expr::MathCall(_, arguments)
            | Expr::ConstructorCall(_, arguments)
            | Expr::ConditionalCall(_, arguments) => {
                for expr in arguments {
                    self.collect_expr(expr, target, offset, needed)?;
                }
            }
            Expr::Nz(value, replacement) | Expr::Weekday(value, replacement) => {
                self.collect_expr(value, target, offset, needed)?;
                if let Some(expr) = replacement {
                    self.collect_expr(expr, target, offset, needed)?;
                }
            }
            Expr::InList(value, list, _) => {
                self.collect_expr(value, target, offset, needed)?;
                for expr in list {
                    self.collect_expr(expr, target, offset, needed)?;
                }
            }
            Expr::Aggregate(_, Some(expr)) => self.collect_expr(expr, target, offset, needed)?,
            Expr::Subquery(read) | Expr::Exists(read) => {
                self.outer_read(read, target + 1, offset, needed)?
            }
            Expr::InQuery(expr, read, _) | Expr::Quantified(expr, _, _, read) => {
                self.collect_expr(expr, target, offset, needed)?;
                self.outer_read(read, target + 1, offset, needed)?;
            }
            Expr::Field { .. }
            | Expr::Literal(_)
            | Expr::Output(_)
            | Expr::Parameter(_)
            | Expr::Aggregate(_, None) => {}
        }
        Ok(())
    }
    fn outer_read(
        &mut self,
        plan: &ReadPlan,
        target: usize,
        offset: usize,
        needed: &mut [bool],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match &plan.node {
            ReadNode::Select(select) => {
                for expr in select
                    .projection
                    .iter()
                    .chain(&select.record_keys)
                    .chain(select.filter.iter())
                    .chain(&select.group)
                    .chain(select.having.iter())
                {
                    self.collect_expr(expr, target, offset, needed)?;
                }
                if let Some(parent) = &select.parent {
                    self.collect_parent(&parent.predicate, target, offset, needed)?;
                }
                self.outer_relation(&select.source, target, offset, needed)?;
            }
            ReadNode::Union { first, rest } => {
                self.outer_read(first, target, offset, needed)?;
                for (_, arm) in rest {
                    self.outer_read(arm, target, offset, needed)?;
                }
            }
        }
        for (expr, _) in &plan.sort {
            self.collect_expr(expr, target, offset, needed)?;
        }
        Ok(())
    }
    fn outer_relation(
        &mut self,
        relation: &Relation,
        target: usize,
        offset: usize,
        needed: &mut [bool],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match &relation.node {
            RelationNode::Cross(left, right) => {
                self.outer_relation(left, target, offset, needed)?;
                self.outer_relation(right, target, offset, needed)?;
            }
            RelationNode::Join {
                left,
                right,
                condition,
                ..
            } => {
                self.outer_relation(left, target, offset, needed)?;
                self.outer_relation(right, target, offset, needed)?;
                self.collect_expr(condition, target, offset, needed)?;
            }
            // Derived inputs are non-lateral and have independent outer scopes.
            RelationNode::Unit | RelationNode::Table { .. } | RelationNode::Derived(_) => {}
        }
        Ok(())
    }
    pub(super) fn local_relation(
        &mut self,
        relation: &Relation,
        offset: usize,
        needed: &mut [bool],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match &relation.node {
            RelationNode::Derived(_) => {
                // Preserve all already-bound derived output expressions/errors.
                needed
                    .get_mut(offset..offset + relation.row_width())
                    .ok_or(QueryError::Source)?
                    .fill(true);
            }
            RelationNode::Cross(left, right) => {
                self.local_relation(left, offset, needed)?;
                self.local_relation(right, offset + left.row_width(), needed)?;
            }
            RelationNode::Join {
                left,
                right,
                condition,
                ..
            } => {
                self.local_relation(left, offset, needed)?;
                self.local_relation(right, offset + left.row_width(), needed)?;
                self.collect_expr(condition, 0, offset, needed)?;
            }
            RelationNode::Unit | RelationNode::Table { .. } => {}
        }
        Ok(())
    }
}
