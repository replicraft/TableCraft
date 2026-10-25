//! Bounded admission of row-local plans before any progressive source callback.
use super::super::Runtime;
use crate::{
    QueryError,
    plan::{Aggregate, Expr, ReadNode, ReadPlan, SelectPlan},
    profile::{MAX_BIND_DEPTH, MAX_BIND_NODES},
};
use tablecraft_expressions::functions::math::MathFunction;

impl Runtime<'_, '_> {
    pub(super) fn progressive_select<'p>(
        &mut self,
        read: &'p ReadPlan,
    ) -> Result<(&'p SelectPlan, Vec<&'p Expr>), QueryError> {
        let select = self.progressive_shape(read)?;
        let mut aggregates = Vec::new();
        if let Some(filter) = &select.filter {
            self.progressive_expression(filter, false, 1, &mut aggregates)?;
        }
        for key in &select.group {
            self.progressive_expression(key, false, 1, &mut aggregates)?;
        }
        for expression in &select.projection {
            self.progressive_expression(expression, select.aggregate, 1, &mut aggregates)?;
        }
        if let Some(having) = &select.having {
            if !select.aggregate {
                return Err(QueryError::Unsupported);
            }
            // Validate the whole predicate before cursors. Its occurrence
            // registry is separate from deferred selected aggregates.
            self.progressive_having_aggregates(having)?;
        }
        Ok((select, aggregates))
    }

    pub(super) fn progressive_inner_select<'p>(
        &mut self,
        read: &'p ReadPlan,
    ) -> Result<&'p SelectPlan, QueryError> {
        let select = self.progressive_shape(read)?;
        if select.aggregate || select.having.is_some() {
            let (select, _) = self.progressive_select(read)?;
            // Stateful inner stages use the same complete metadata profile as
            // terminal services. No records or inferred first-row types bind it.
            self.progressive_group_lookup(select)?;
            return Ok(select);
        }
        let mut aggregates = Vec::new();
        if let Some(filter) = &select.filter {
            self.progressive_expression(filter, false, 1, &mut aggregates)?;
        }
        for expression in &select.projection {
            self.progressive_expression(expression, false, 1, &mut aggregates)?;
        }
        Ok(select)
    }

    fn progressive_shape<'p>(&mut self, read: &'p ReadPlan) -> Result<&'p SelectPlan, QueryError> {
        self.tick()?;
        let ReadNode::Select(select) = &read.node else {
            return Err(QueryError::Unsupported);
        };
        if !read.sort.is_empty()
            || read.distinct
            || read.top.is_some_and(|(_, percent)| percent)
            || !select.record_keys.is_empty()
            || select.parent.is_some()
        {
            return Err(QueryError::Unsupported);
        }
        Ok(select)
    }

    pub(super) fn progressive_having_aggregates<'p>(
        &mut self,
        expression: &'p Expr,
    ) -> Result<Vec<&'p Expr>, QueryError> {
        let mut aggregates = Vec::new();
        self.progressive_expression(expression, true, 1, &mut aggregates)?;
        Ok(aggregates)
    }

    fn progressive_expression<'p>(
        &mut self,
        expression: &'p Expr,
        count: bool,
        depth: usize,
        aggregates: &mut Vec<&'p Expr>,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if depth > MAX_BIND_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        match expression {
            Expr::Aggregate(
                Aggregate::Count
                | Aggregate::Sum
                | Aggregate::Average
                | Aggregate::Minimum
                | Aggregate::Maximum,
                argument,
            ) if count => {
                if aggregates.len() >= MAX_BIND_NODES {
                    return Err(QueryError::StructureLimit);
                }
                if aggregates.len() == aggregates.capacity() {
                    let additional = aggregates
                        .len()
                        .max(1)
                        .min(MAX_BIND_NODES - aggregates.len());
                    self.budget
                        .charge_allocation(additional as u64, size_of::<&Expr>() as u64)?;
                    aggregates
                        .try_reserve_exact(additional)
                        .map_err(|_| QueryError::Allocation)?;
                }
                aggregates.push(expression);
                if let Some(argument) = argument {
                    self.progressive_expression(argument, false, depth + 1, aggregates)?;
                }
                Ok(())
            }
            Expr::Literal(_)
            | Expr::NumericLiteral(_)
            | Expr::DateLiteral(..)
            | Expr::Field { depth: 0, .. }
            | Expr::Parameter(_)
            | Expr::CurrentUser
            | Expr::Projected(_) => Ok(()),
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
            | Expr::DateConvert(_, value) => {
                self.progressive_expression(value, count, depth + 1, aggregates)
            }
            Expr::Binary(_, left, right)
            | Expr::Like(left, right, _)
            | Expr::ArrayBound(_, left, right) => {
                self.progressive_expression(left, count, depth + 1, aggregates)?;
                self.progressive_expression(right, count, depth + 1, aggregates)
            }
            Expr::IIf(first, second, third)
            | Expr::DateAdd(first, second, third)
            | Expr::Between(first, second, third, _) => {
                self.progressive_expression(first, count, depth + 1, aggregates)?;
                self.progressive_expression(second, count, depth + 1, aggregates)?;
                self.progressive_expression(third, count, depth + 1, aggregates)
            }
            Expr::DateDiff(first, second, third, options) => {
                self.progressive_expression(first, count, depth + 1, aggregates)?;
                self.progressive_expression(second, count, depth + 1, aggregates)?;
                self.progressive_expression(third, count, depth + 1, aggregates)?;
                for option in options.iter().flatten() {
                    self.progressive_expression(option, count, depth + 1, aggregates)?;
                }
                Ok(())
            }
            Expr::DatePart(first, second, options) => {
                self.progressive_expression(first, count, depth + 1, aggregates)?;
                self.progressive_expression(second, count, depth + 1, aggregates)?;
                for option in options.iter().flatten() {
                    self.progressive_expression(option, count, depth + 1, aggregates)?;
                }
                Ok(())
            }
            Expr::Nz(value, option) | Expr::Weekday(value, option) => {
                self.progressive_expression(value, count, depth + 1, aggregates)?;
                if let Some(option) = option {
                    self.progressive_expression(option, count, depth + 1, aggregates)?;
                }
                Ok(())
            }
            Expr::InList(value, items, _) => {
                self.progressive_expression(value, count, depth + 1, aggregates)?;
                self.progressive_arguments(items, count, depth + 1, aggregates)
            }
            Expr::MathCall(MathFunction::Rnd, _) => Err(QueryError::Unsupported),
            Expr::TextCall(_, arguments)
            | Expr::MathCall(_, arguments)
            | Expr::FinancialCall(_, arguments)
            | Expr::CashFlowCall(_, arguments, _)
            | Expr::ConstructorCall(_, arguments)
            | Expr::ConditionalCall(_, arguments)
            | Expr::Array(arguments) => {
                self.progressive_arguments(arguments, count, depth + 1, aggregates)
            }
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

    fn progressive_arguments<'p>(
        &mut self,
        arguments: &'p [Expr],
        count: bool,
        depth: usize,
        aggregates: &mut Vec<&'p Expr>,
    ) -> Result<(), QueryError> {
        for argument in arguments {
            self.progressive_expression(argument, count, depth, aggregates)?;
        }
        Ok(())
    }
}
