//! Validate all captures of a grouped frame, including nested read predicates.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ParentPredicate, ReadNode, ReadPlan, Relation, RelationNode},
};

impl Binder<'_> {
    pub(in crate::binding) fn validate_group(
        &mut self,
        expr: &Expr,
        keys: &[Expr],
        depth: usize,
    ) -> Result<(), QueryError> {
        self.validate_group_at(expr, keys, depth, 0)
    }

    fn validate_group_at(
        &mut self,
        expr: &Expr,
        keys: &[Expr],
        depth: usize,
        scope: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        for key in keys {
            if self.same_group_expression_at(expr, key, depth + 1, scope)? {
                return Ok(());
            }
        }
        match expr {
            Expr::Field { depth, .. } | Expr::CollectionItem { depth, .. } if *depth == scope => {
                return Err(QueryError::Grouping);
            }
            Expr::Unary(_, e)
            | Expr::IsNull(e, _)
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
            | Expr::DateConvert(_, e) => self.validate_group_at(e, keys, depth + 1, scope)?,
            Expr::Binary(_, a, b) | Expr::ArrayBound(_, a, b) | Expr::Like(a, b, _) => {
                self.validate_group_at(a, keys, depth + 1, scope)?;
                self.validate_group_at(b, keys, depth + 1, scope)?;
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                self.validate_group_at(a, keys, depth + 1, scope)?;
                self.validate_group_at(b, keys, depth + 1, scope)?;
                self.validate_group_at(c, keys, depth + 1, scope)?;
            }
            Expr::DateDiff(a, b, c, options) => {
                for expr in [a, b, c].into_iter().chain(options.iter().flatten()) {
                    self.validate_group_at(expr, keys, depth + 1, scope)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for expr in [a, b].into_iter().chain(options.iter().flatten()) {
                    self.validate_group_at(expr, keys, depth + 1, scope)?;
                }
            }
            Expr::DomainCall(_, arguments)
            | Expr::TextCall(_, arguments)
            | Expr::FinancialCall(_, arguments)
            | Expr::Array(arguments)
            | Expr::CashFlowCall(_, arguments, _)
            | Expr::MathCall(_, arguments)
            | Expr::ConstructorCall(_, arguments)
            | Expr::ConditionalCall(_, arguments) => {
                for expr in arguments {
                    self.validate_group_at(expr, keys, depth + 1, scope)?;
                }
            }
            Expr::Nz(value, replacement) | Expr::Weekday(value, replacement) => {
                self.validate_group_at(value, keys, depth + 1, scope)?;
                if let Some(expr) = replacement {
                    self.validate_group_at(expr, keys, depth + 1, scope)?;
                }
            }
            Expr::InList(e, list, _) => {
                self.validate_group_at(e, keys, depth + 1, scope)?;
                for e in list {
                    self.validate_group_at(e, keys, depth + 1, scope)?;
                }
            }
            Expr::Subquery(read) | Expr::Exists(read) => {
                self.group_read(read, keys, depth + 1, scope + 1)?;
            }
            Expr::InQuery(value, read, _) | Expr::Quantified(value, _, _, read) => {
                self.validate_group_at(value, keys, depth + 1, scope)?;
                self.group_read(read, keys, depth + 1, scope + 1)?;
            }
            Expr::Aggregate(_, Some(argument)) if scope > 0 => {
                self.validate_group_at(argument, keys, depth + 1, scope)?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl Binder<'_> {
    fn group_read(
        &mut self,
        read: &ReadPlan,
        keys: &[Expr],
        depth: usize,
        scope: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        for (expression, _) in &read.sort {
            self.validate_group_at(expression, keys, depth + 1, scope)?;
        }
        match &read.node {
            ReadNode::Select(select) => {
                self.group_relation(&select.source, keys, depth + 1, scope)?;
                for expression in select
                    .projection
                    .iter()
                    .chain(select.filter.iter())
                    .chain(&select.group)
                    .chain(select.having.iter())
                    .chain(&select.record_keys)
                {
                    self.validate_group_at(expression, keys, depth + 1, scope)?;
                }
                if let Some(parent) = &select.parent {
                    self.group_parent(&parent.predicate, keys, depth + 1, scope)?;
                }
            }
            ReadNode::Union { first, rest } => {
                self.group_read(first, keys, depth + 1, scope)?;
                for (_, arm) in rest {
                    self.group_read(arm, keys, depth + 1, scope)?;
                }
            }
            // Named crosstabs are isolated definitions, not lateral reads.
            ReadNode::Crosstab(_) => {}
        }
        Ok(())
    }

    fn group_relation(
        &mut self,
        relation: &Relation,
        keys: &[Expr],
        depth: usize,
        scope: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match &relation.node {
            RelationNode::Cross(left, right) => {
                self.group_relation(left, keys, depth + 1, scope)?;
                self.group_relation(right, keys, depth + 1, scope)?;
            }
            RelationNode::Join {
                left,
                right,
                condition,
                ..
            } => {
                self.group_relation(left, keys, depth + 1, scope)?;
                self.group_relation(right, keys, depth + 1, scope)?;
                self.validate_group_at(condition, keys, depth + 1, scope)?;
            }
            // Derived and named relations bind/run in isolated, non-lateral scopes.
            RelationNode::Derived(_) | RelationNode::Table { .. } | RelationNode::Unit => {}
        }
        Ok(())
    }

    fn group_parent(
        &mut self,
        predicate: &ParentPredicate,
        keys: &[Expr],
        depth: usize,
        scope: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match predicate {
            ParentPredicate::Scalar(expression)
            | ParentPredicate::Any {
                predicate: expression,
                ..
            } => {
                self.validate_group_at(expression, keys, depth + 1, scope)?;
            }
            ParentPredicate::AnyMany(product) => {
                self.validate_group_at(&product.predicate, keys, depth + 1, scope)?;
            }
            ParentPredicate::Not(predicate) => {
                self.group_parent(predicate, keys, depth + 1, scope)?
            }
            ParentPredicate::Logical(_, left, right) => {
                self.group_parent(left, keys, depth + 1, scope)?;
                self.group_parent(right, keys, depth + 1, scope)?;
            }
        }
        Ok(())
    }
}
