//! Rebind one hidden member through correlated scopes without output aliases.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ParentPredicate, ReadNode, ReadPlan, Relation, RelationNode},
};

impl Binder<'_> {
    pub(super) fn replace_item(
        &mut self,
        expr: &mut Expr,
        field: usize,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Field {
                depth: scope,
                index,
            } if *scope == target && *index == field => {
                *expr = Expr::CollectionItem(target);
                *capture |= target != 0;
            }
            Expr::Unary(_, e)
            | Expr::IsNull(e, _)
            | Expr::DateCall(_, e)
            | Expr::Convert(_, e)
            | Expr::Inspect(_, e)
            | Expr::DateConvert(_, e) => self.replace_item(e, field, target, capture, depth + 1)?,
            Expr::Binary(_, a, b) | Expr::Like(a, b, _) => {
                self.replace_item(a, field, target, capture, depth + 1)?;
                self.replace_item(b, field, target, capture, depth + 1)?;
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                for e in [a, b, c] {
                    self.replace_item(e, field, target, capture, depth + 1)?;
                }
            }
            Expr::DateDiff(a, b, c, options) => {
                for e in [a, b, c].into_iter().chain(options.iter_mut().flatten()) {
                    self.replace_item(e, field, target, capture, depth + 1)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for e in [a, b].into_iter().chain(options.iter_mut().flatten()) {
                    self.replace_item(e, field, target, capture, depth + 1)?;
                }
            }
            Expr::TextCall(_, args)
            | Expr::MathCall(_, args)
            | Expr::ConstructorCall(_, args)
            | Expr::ConditionalCall(_, args) => {
                for e in args {
                    self.replace_item(e, field, target, capture, depth + 1)?;
                }
            }
            Expr::Nz(value, replacement) | Expr::Weekday(value, replacement) => {
                self.replace_item(value, field, target, capture, depth + 1)?;
                if let Some(e) = replacement {
                    self.replace_item(e, field, target, capture, depth + 1)?;
                }
            }
            Expr::InList(value, list, _) => {
                self.replace_item(value, field, target, capture, depth + 1)?;
                for e in list {
                    self.replace_item(e, field, target, capture, depth + 1)?;
                }
            }
            Expr::Clock(_)
            | Expr::Literal(_)
            | Expr::Field { .. }
            | Expr::Parameter(_)
            | Expr::Output(_)
            | Expr::CollectionItem(_) => {}
            Expr::Subquery(read) | Expr::Exists(read) => {
                self.replace_item_read(read, field, target + 1, capture, depth + 1)?;
            }
            Expr::InQuery(value, read, _) | Expr::Quantified(value, _, _, read) => {
                self.replace_item(value, field, target, capture, depth + 1)?;
                self.replace_item_read(read, field, target + 1, capture, depth + 1)?;
            }
            Expr::Aggregate(_, value) => {
                if let Some(value) = value {
                    self.replace_item(value, field, target, capture, depth + 1)?;
                }
            }
        }
        Ok(())
    }
    fn replace_item_read(
        &mut self,
        read: &mut ReadPlan,
        field: usize,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match &mut read.node {
            // Named crosstabs and derived sources have independent scopes.
            ReadNode::Crosstab(_) => return Ok(()),
            ReadNode::Union { first, rest } => {
                self.replace_item_read(first, field, target, capture, depth + 1)?;
                for (_, arm) in rest {
                    self.replace_item_read(arm, field, target, capture, depth + 1)?;
                }
            }
            ReadNode::Select(select) => {
                for expr in select
                    .projection
                    .iter_mut()
                    .chain(&mut select.record_keys)
                    .chain(select.filter.iter_mut())
                    .chain(&mut select.group)
                    .chain(select.having.iter_mut())
                {
                    self.replace_item(expr, field, target, capture, depth + 1)?;
                }
                if let Some(parent) = &mut select.parent {
                    self.replace_item_parent(
                        &mut parent.predicate,
                        field,
                        target,
                        capture,
                        depth + 1,
                    )?;
                }
                self.replace_item_relation(&mut select.source, field, target, capture, depth + 1)?;
            }
        }
        for (expr, _) in &mut read.sort {
            self.replace_item(expr, field, target, capture, depth + 1)?;
        }
        Ok(())
    }
    fn replace_item_parent(
        &mut self,
        predicate: &mut ParentPredicate,
        field: usize,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match predicate {
            ParentPredicate::Scalar(expr)
            | ParentPredicate::Any {
                predicate: expr, ..
            } => self.replace_item(expr, field, target, capture, depth + 1)?,
            ParentPredicate::Not(value) => {
                self.replace_item_parent(value, field, target, capture, depth + 1)?
            }
            ParentPredicate::Logical(_, a, b) => {
                self.replace_item_parent(a, field, target, capture, depth + 1)?;
                self.replace_item_parent(b, field, target, capture, depth + 1)?;
            }
        }
        Ok(())
    }
    fn replace_item_relation(
        &mut self,
        relation: &mut Relation,
        field: usize,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match &mut relation.node {
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                self.replace_item_relation(left, field, target, capture, depth + 1)?;
                self.replace_item_relation(right, field, target, capture, depth + 1)?;
                if let RelationNode::Join { condition, .. } = &mut relation.node {
                    self.replace_item(condition, field, target, capture, depth + 1)?;
                }
            }
            RelationNode::Unit | RelationNode::Table { .. } | RelationNode::Derived(_) => {}
        }
        Ok(())
    }
}
