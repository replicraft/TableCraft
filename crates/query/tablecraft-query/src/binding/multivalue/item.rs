//! Rebind one hidden member through correlated scopes without output aliases.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ParentPredicate, ReadNode, ReadPlan, Relation, RelationNode},
};

pub(super) struct ItemBinding {
    pub field: usize,
    pub index: usize,
    pub width: usize,
}

impl Binder<'_> {
    pub(super) fn replace_item(
        &mut self,
        expr: &mut Expr,
        binding: &ItemBinding,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Field {
                depth: scope,
                index,
                text_policy,
            } if *scope == target && *index == binding.field => {
                *expr = Expr::CollectionItem {
                    text_policy: *text_policy,
                    depth: target,
                    index: binding.index,
                    width: binding.width,
                };
                *capture |= target != 0;
            }
            Expr::Unary(_, e)
            | Expr::IsNull(e, _)
            | Expr::CollectionEmpty(e, _)
            | Expr::DateCall(_, e)
            | Expr::Convert(_, e)
            | Expr::CVar(e)
            | Expr::CvErr(e)
            | Expr::Eval(e)
            | Expr::GuidFromString(e)
            | Expr::ErrorMessage(e)
            | Expr::ErrorMessageString(e)
            | Expr::StringFromGuid(e)
            | Expr::Inspect(_, e)
            | Expr::ValueInspect(_, e)
            | Expr::DateConvert(_, e) => {
                self.replace_item(e, binding, target, capture, depth + 1)?
            }
            Expr::Binary(_, a, b) | Expr::ArrayBound(_, a, b) | Expr::Like(a, b, _) => {
                self.replace_item(a, binding, target, capture, depth + 1)?;
                self.replace_item(b, binding, target, capture, depth + 1)?;
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                for e in [a, b, c] {
                    self.replace_item(e, binding, target, capture, depth + 1)?;
                }
            }
            Expr::DateDiff(a, b, c, options) => {
                for e in [a, b, c].into_iter().chain(options.iter_mut().flatten()) {
                    self.replace_item(e, binding, target, capture, depth + 1)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for e in [a, b].into_iter().chain(options.iter_mut().flatten()) {
                    self.replace_item(e, binding, target, capture, depth + 1)?;
                }
            }
            Expr::DomainCall(_, args)
            | Expr::TextCall(_, args)
            | Expr::FinancialCall(_, args)
            | Expr::Array(args)
            | Expr::CashFlowCall(_, args, _)
            | Expr::MathCall(_, args)
            | Expr::ConstructorCall(_, args)
            | Expr::ConditionalCall(_, args) => {
                for e in args {
                    self.replace_item(e, binding, target, capture, depth + 1)?;
                }
            }
            Expr::Nz(value, replacement) | Expr::Weekday(value, replacement) => {
                self.replace_item(value, binding, target, capture, depth + 1)?;
                if let Some(e) = replacement {
                    self.replace_item(e, binding, target, capture, depth + 1)?;
                }
            }
            Expr::InList(value, list, _) => {
                self.replace_item(value, binding, target, capture, depth + 1)?;
                for e in list {
                    self.replace_item(e, binding, target, capture, depth + 1)?;
                }
            }
            Expr::Multivalue { .. } => return Err(QueryError::Unsupported),
            Expr::Clock(_)
            | Expr::CurrentUser
            | Expr::Literal(_)
            | Expr::NumericLiteral(_)
            | Expr::DateLiteral(..)
            | Expr::Field { .. }
            | Expr::Parameter(_)
            | Expr::Output(_, _)
            | Expr::Projected(_)
            | Expr::CollectionItem { .. }
            | Expr::ParentPresence { .. } => {}
            Expr::Subquery(read) | Expr::Exists(read) => {
                self.replace_item_read(read, binding, target + 1, capture, depth + 1)?;
            }
            Expr::InQuery(value, read, _) | Expr::Quantified(value, _, _, read) => {
                self.replace_item(value, binding, target, capture, depth + 1)?;
                self.replace_item_read(read, binding, target + 1, capture, depth + 1)?;
            }
            Expr::Aggregate(_, value) => {
                if let Some(value) = value {
                    self.replace_item(value, binding, target, capture, depth + 1)?;
                }
            }
        }
        Ok(())
    }
    fn replace_item_read(
        &mut self,
        read: &mut ReadPlan,
        binding: &ItemBinding,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match &mut read.node {
            // Named crosstabs and derived sources have independent scopes.
            ReadNode::Crosstab(_) => return Ok(()),
            ReadNode::Union { first, rest } => {
                self.replace_item_read(first, binding, target, capture, depth + 1)?;
                for (_, arm) in rest {
                    self.replace_item_read(arm, binding, target, capture, depth + 1)?;
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
                    self.replace_item(expr, binding, target, capture, depth + 1)?;
                }
                if let Some(parent) = &mut select.parent {
                    self.replace_item_parent(
                        &mut parent.predicate,
                        binding,
                        target,
                        capture,
                        depth + 1,
                    )?;
                }
                self.replace_item_relation(
                    &mut select.source,
                    binding,
                    target,
                    capture,
                    depth + 1,
                )?;
            }
        }
        for (expr, _) in &mut read.sort {
            self.replace_item(expr, binding, target, capture, depth + 1)?;
        }
        Ok(())
    }
    fn replace_item_parent(
        &mut self,
        predicate: &mut ParentPredicate,
        binding: &ItemBinding,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match predicate {
            ParentPredicate::Empty { .. } => {}
            ParentPredicate::Scalar(expr)
            | ParentPredicate::Any {
                predicate: expr, ..
            } => self.replace_item(expr, binding, target, capture, depth + 1)?,
            ParentPredicate::AnyMany(product) => {
                self.replace_item(&mut product.predicate, binding, target, capture, depth + 1)?
            }
            ParentPredicate::Not(value) => {
                self.replace_item_parent(value, binding, target, capture, depth + 1)?
            }
            ParentPredicate::Logical(_, a, b) => {
                self.replace_item_parent(a, binding, target, capture, depth + 1)?;
                self.replace_item_parent(b, binding, target, capture, depth + 1)?;
            }
        }
        Ok(())
    }
    fn replace_item_relation(
        &mut self,
        relation: &mut Relation,
        binding: &ItemBinding,
        target: usize,
        capture: &mut bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match &mut relation.node {
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                self.replace_item_relation(left, binding, target, capture, depth + 1)?;
                self.replace_item_relation(right, binding, target, capture, depth + 1)?;
                if let RelationNode::Join { condition, .. } = &mut relation.node {
                    self.replace_item(condition, binding, target, capture, depth + 1)?;
                }
            }
            RelationNode::Unit | RelationNode::Table { .. } | RelationNode::Derived(_) => {}
        }
        Ok(())
    }
}
