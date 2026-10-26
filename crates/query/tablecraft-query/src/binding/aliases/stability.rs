//! Conservative stability admission never evaluates an alias definition.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ReadNode, ReadPlan, Relation, RelationNode},
};
use tablecraft_expressions::functions::math::MathFunction;

impl Binder<'_> {
    pub(super) fn stable_alias_expression(
        &mut self,
        expr: &Expr,
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        Ok(match expr {
            Expr::Multivalue { .. }
            | Expr::MathCall(MathFunction::Rnd, _)
            | Expr::Clock(_)
            | Expr::Eval(_)
            | Expr::DomainCall(..) => false,
            Expr::Literal(_)
            | Expr::NumericLiteral(_)
            | Expr::DateLiteral(..)
            | Expr::Field { .. }
            | Expr::Parameter(_)
            | Expr::CollectionItem { .. }
            | Expr::ParentPresence { .. }
            | Expr::Output(_, _)
            | Expr::Projected(_)
            | Expr::CurrentUser => true,
            Expr::Unary(_, expr)
            | Expr::IsNull(expr, _)
            | Expr::CollectionEmpty(expr, _)
            | Expr::DateCall(_, expr)
            | Expr::Convert(_, expr)
            | Expr::CVar(expr)
            | Expr::CvErr(expr)
            | Expr::GuidFromString(expr)
            | Expr::ErrorMessage(expr)
            | Expr::ErrorMessageString(expr)
            | Expr::StringFromGuid(expr)
            | Expr::Inspect(_, expr)
            | Expr::ValueInspect(_, expr)
            | Expr::DateConvert(_, expr) => self.stable_alias_expression(expr, depth + 1)?,
            Expr::Binary(_, a, b) | Expr::ArrayBound(_, a, b) | Expr::Like(a, b, _) => {
                self.stable_alias_children([a.as_ref(), b.as_ref()], depth + 1)?
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                self.stable_alias_children([a.as_ref(), b.as_ref(), c.as_ref()], depth + 1)?
            }
            Expr::DateDiff(a, b, c, options) => self.stable_alias_children(
                [a, b, c]
                    .into_iter()
                    .chain(options.iter().flatten())
                    .map(Box::as_ref),
                depth + 1,
            )?,
            Expr::DatePart(a, b, options) => self.stable_alias_children(
                [a, b]
                    .into_iter()
                    .chain(options.iter().flatten())
                    .map(Box::as_ref),
                depth + 1,
            )?,
            Expr::TextCall(_, args)
            | Expr::FinancialCall(_, args)
            | Expr::Array(args)
            | Expr::CashFlowCall(_, args, _)
            | Expr::MathCall(_, args)
            | Expr::ConstructorCall(_, args)
            | Expr::ConditionalCall(_, args) => self.stable_alias_children(args, depth + 1)?,
            Expr::Nz(value, option) | Expr::Weekday(value, option) => self.stable_alias_children(
                std::iter::once(value).chain(option.iter()).map(Box::as_ref),
                depth + 1,
            )?,
            Expr::InList(value, list, _) => self.stable_alias_children(
                std::iter::once(value.as_ref()).chain(list.iter()),
                depth + 1,
            )?,
            Expr::Aggregate(_, argument) => {
                self.stable_alias_children(argument.iter().map(Box::as_ref), depth + 1)?
            }
            Expr::Subquery(read) | Expr::Exists(read) => self.stable_alias_read(read, depth + 1)?,
            Expr::InQuery(value, read, _) | Expr::Quantified(value, _, _, read) => {
                self.stable_alias_expression(value, depth + 1)?
                    && self.stable_alias_read(read, depth + 1)?
            }
        })
    }

    fn stable_alias_children<'a>(
        &mut self,
        expressions: impl IntoIterator<Item = &'a Expr>,
        depth: usize,
    ) -> Result<bool, QueryError> {
        for expression in expressions {
            if !self.stable_alias_expression(expression, depth)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn stable_alias_read(&mut self, read: &ReadPlan, depth: usize) -> Result<bool, QueryError> {
        self.tick(depth)?;
        for (expression, _) in &read.sort {
            if !self.stable_alias_expression(expression, depth + 1)? {
                return Ok(false);
            }
        }
        match &read.node {
            ReadNode::Select(select) => {
                if select.parent.is_some()
                    || !self.stable_alias_relation(&select.source, depth + 1)?
                {
                    return Ok(false);
                }
                self.stable_alias_children(
                    select
                        .projection
                        .iter()
                        .chain(select.filter.iter())
                        .chain(select.group.iter())
                        .chain(select.having.iter())
                        .chain(select.record_keys.iter()),
                    depth + 1,
                )
            }
            ReadNode::Union { first, rest } => {
                if !self.stable_alias_read(first, depth + 1)? {
                    return Ok(false);
                }
                for (_, arm) in rest {
                    if !self.stable_alias_read(arm, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ReadNode::Crosstab(_) => Ok(false),
        }
    }

    fn stable_alias_relation(
        &mut self,
        relation: &Relation,
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        match &relation.node {
            RelationNode::Unit | RelationNode::Table { .. } => Ok(true),
            RelationNode::Cross(left, right) => Ok(self.stable_alias_relation(left, depth + 1)?
                && self.stable_alias_relation(right, depth + 1)?),
            RelationNode::Join {
                left,
                right,
                condition,
                ..
            } => Ok(self.stable_alias_relation(left, depth + 1)?
                && self.stable_alias_relation(right, depth + 1)?
                && self.stable_alias_expression(condition, depth + 1)?),
            RelationNode::Derived { read, .. } => self.stable_alias_read(read, depth + 1),
        }
    }
}
