//! Group membership uses bound expression structure, never SQL algebraic rewrites.
mod validation;

use super::Binder;
use crate::{QueryError, plan::Expr};
use tablecraft_expressions::functions::math::MathFunction;

impl Binder<'_> {
    pub(super) fn same_group_expression(
        &mut self,
        left: &Expr,
        right: &Expr,
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.same_group_expression_at(left, right, depth, 0)
    }

    /// Key eligibility is separate from equivalence: clocks can produce keys,
    /// but two clock calls are never assumed to return the same value.
    pub(super) fn scalar_group_key(
        &mut self,
        expr: &Expr,
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        Ok(match expr {
            Expr::MathCall(MathFunction::Rnd, _) => false,
            Expr::Literal(_)
            | Expr::DateLiteral(..)
            | Expr::Field { .. }
            | Expr::Parameter(_)
            | Expr::CollectionItem { .. }
            | Expr::Output(_, _)
            | Expr::Clock(_)
            | Expr::CurrentUser => true,
            Expr::Unary(_, expr)
            | Expr::IsNull(expr, _)
            | Expr::DateCall(_, expr)
            | Expr::Convert(_, expr)
            | Expr::CVar(expr)
            | Expr::CvErr(expr)
            | Expr::Eval(expr)
            | Expr::GuidFromString(expr)
            | Expr::ErrorMessage(expr)
            | Expr::ErrorMessageString(expr)
            | Expr::StringFromGuid(expr)
            | Expr::Inspect(_, expr)
            | Expr::ValueInspect(_, expr)
            | Expr::DateConvert(_, expr) => self.scalar_group_key(expr, depth + 1)?,
            Expr::Binary(_, a, b) | Expr::ArrayBound(_, a, b) | Expr::Like(a, b, _) => {
                self.group_key_children([a.as_ref(), b.as_ref()], depth + 1)?
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                self.group_key_children([a.as_ref(), b.as_ref(), c.as_ref()], depth + 1)?
            }
            Expr::DateDiff(a, b, c, options) => self.group_key_children(
                [a, b, c]
                    .into_iter()
                    .chain(options.iter().flatten())
                    .map(Box::as_ref),
                depth + 1,
            )?,
            Expr::DatePart(a, b, options) => self.group_key_children(
                [a, b]
                    .into_iter()
                    .chain(options.iter().flatten())
                    .map(Box::as_ref),
                depth + 1,
            )?,
            Expr::DomainCall(_, args)
            | Expr::TextCall(_, args)
            | Expr::FinancialCall(_, args)
            | Expr::Array(args)
            | Expr::CashFlowCall(_, args, _)
            | Expr::MathCall(_, args)
            | Expr::ConstructorCall(_, args)
            | Expr::ConditionalCall(_, args) => self.group_key_children(args, depth + 1)?,
            Expr::Nz(value, option) | Expr::Weekday(value, option) => self.group_key_children(
                std::iter::once(value).chain(option.iter()).map(Box::as_ref),
                depth + 1,
            )?,
            Expr::InList(value, list, _) => self.group_key_children(
                std::iter::once(value.as_ref()).chain(list.iter()),
                depth + 1,
            )?,
            Expr::Aggregate(..)
            | Expr::Subquery(_)
            | Expr::Exists(_)
            | Expr::InQuery(..)
            | Expr::Quantified(..) => false,
        })
    }
    fn group_key_children<'a>(
        &mut self,
        expressions: impl IntoIterator<Item = &'a Expr>,
        depth: usize,
    ) -> Result<bool, QueryError> {
        for expression in expressions {
            if !self.scalar_group_key(expression, depth)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
