//! Bounded admission of row-local plans before any progressive source callback.
use super::super::Runtime;
use crate::{
    QueryError,
    external_sources::SourceNamespace,
    plan::{Expr, ReadNode, ReadPlan, RelationNode, SelectPlan},
    profile::MAX_BIND_DEPTH,
};
use tablecraft_expressions::functions::math::MathFunction;

impl Runtime<'_, '_> {
    pub(super) fn progressive_select<'p>(
        &mut self,
        read: &'p ReadPlan,
    ) -> Result<&'p SelectPlan, QueryError> {
        self.tick()?;
        let ReadNode::Select(select) = &read.node else {
            return Err(QueryError::Unsupported);
        };
        if read.owner != SourceNamespace::Root
            || !read.sort.is_empty()
            || read.distinct
            || read.top.is_some_and(|(_, percent)| percent)
            || select.aggregate
            || !select.group.is_empty()
            || select.having.is_some()
            || !select.record_keys.is_empty()
            || select.parent.is_some()
        {
            return Err(QueryError::Unsupported);
        }
        let RelationNode::Table {
            namespace: SourceNamespace::Root,
            expansion,
            ..
        } = &select.source.node
        else {
            return Err(QueryError::Unsupported);
        };
        if !expansion.is_empty() {
            return Err(QueryError::Unsupported);
        }
        if let Some(filter) = &select.filter {
            self.progressive_expression(filter, 1)?;
        }
        for expression in &select.projection {
            self.progressive_expression(expression, 1)?;
        }
        Ok(select)
    }

    fn progressive_expression(
        &mut self,
        expression: &Expr,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if depth > MAX_BIND_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        match expression {
            Expr::Literal(_)
            | Expr::NumericLiteral(_)
            | Expr::DateLiteral(..)
            | Expr::Field { depth: 0, .. }
            | Expr::Parameter(_)
            | Expr::CurrentUser => Ok(()),
            Expr::Unary(_, value)
            | Expr::IsNull(value, _)
            | Expr::DateCall(_, value)
            | Expr::Convert(_, value)
            | Expr::CVar(value)
            | Expr::CvErr(value)
            | Expr::GuidFromString(value)
            | Expr::StringFromGuid(value)
            | Expr::ErrorMessage(value)
            | Expr::ErrorMessageString(value)
            | Expr::Inspect(_, value)
            | Expr::ValueInspect(_, value)
            | Expr::DateConvert(_, value) => self.progressive_expression(value, depth + 1),
            Expr::Binary(_, left, right)
            | Expr::Like(left, right, _)
            | Expr::ArrayBound(_, left, right) => {
                self.progressive_expression(left, depth + 1)?;
                self.progressive_expression(right, depth + 1)
            }
            Expr::IIf(first, second, third)
            | Expr::DateAdd(first, second, third)
            | Expr::Between(first, second, third, _) => {
                self.progressive_expression(first, depth + 1)?;
                self.progressive_expression(second, depth + 1)?;
                self.progressive_expression(third, depth + 1)
            }
            Expr::DateDiff(first, second, third, options) => {
                self.progressive_expression(first, depth + 1)?;
                self.progressive_expression(second, depth + 1)?;
                self.progressive_expression(third, depth + 1)?;
                for option in options.iter().flatten() {
                    self.progressive_expression(option, depth + 1)?;
                }
                Ok(())
            }
            Expr::DatePart(first, second, options) => {
                self.progressive_expression(first, depth + 1)?;
                self.progressive_expression(second, depth + 1)?;
                for option in options.iter().flatten() {
                    self.progressive_expression(option, depth + 1)?;
                }
                Ok(())
            }
            Expr::Nz(value, option) | Expr::Weekday(value, option) => {
                self.progressive_expression(value, depth + 1)?;
                if let Some(option) = option {
                    self.progressive_expression(option, depth + 1)?;
                }
                Ok(())
            }
            Expr::InList(value, items, _) => {
                self.progressive_expression(value, depth + 1)?;
                self.progressive_arguments(items, depth + 1)
            }
            Expr::MathCall(MathFunction::Rnd, _) => Err(QueryError::Unsupported),
            Expr::TextCall(_, arguments)
            | Expr::MathCall(_, arguments)
            | Expr::FinancialCall(_, arguments)
            | Expr::CashFlowCall(_, arguments, _)
            | Expr::ConstructorCall(_, arguments)
            | Expr::ConditionalCall(_, arguments)
            | Expr::Array(arguments) => self.progressive_arguments(arguments, depth + 1),
            Expr::DomainCall(..)
            | Expr::Field { .. }
            | Expr::Output(..)
            | Expr::CollectionItem { .. }
            | Expr::CollectionEmpty(..)
            | Expr::ParentPresence { .. }
            | Expr::Eval(_)
            | Expr::Clock(_)
            | Expr::Aggregate(..)
            | Expr::Subquery(_)
            | Expr::Exists(_)
            | Expr::InQuery(..)
            | Expr::Quantified(..) => Err(QueryError::Unsupported),
        }
    }

    fn progressive_arguments(
        &mut self,
        arguments: &[Expr],
        depth: usize,
    ) -> Result<(), QueryError> {
        for argument in arguments {
            self.progressive_expression(argument, depth)?;
        }
        Ok(())
    }
}
