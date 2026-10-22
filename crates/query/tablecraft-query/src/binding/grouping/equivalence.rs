//! Compare group-key structure across explicitly shifted query frames.
use super::super::Binder;
use crate::{QueryError, plan::Expr};

impl Binder<'_> {
    pub(super) fn same_group_expression_at(
        &mut self,
        left: &Expr,
        right: &Expr,
        depth: usize,
        scope: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        Ok(match (left, right) {
            (Expr::Literal(a), Expr::Literal(b)) => a == b,
            (Expr::CurrentUser, Expr::CurrentUser) => true,
            (
                Expr::Field {
                    depth: a, index: i, ..
                },
                Expr::Field {
                    depth: b, index: j, ..
                },
            ) => *a == *b + scope && i == j,
            (Expr::Parameter(a), Expr::Parameter(b)) | (Expr::Output(a, _), Expr::Output(b, _)) => {
                a == b
            }
            (
                Expr::CollectionItem {
                    depth: a,
                    index: i,
                    width: x,
                    ..
                },
                Expr::CollectionItem {
                    depth: b,
                    index: j,
                    width: y,
                    ..
                },
            ) => *a == *b + scope && i == j && x == y,
            (Expr::Unary(a, x), Expr::Unary(b, y)) => {
                a == b && self.same_group_expression_at(x, y, depth + 1, scope)?
            }
            (Expr::IsNull(x, a), Expr::IsNull(y, b)) => {
                a == b && self.same_group_expression_at(x, y, depth + 1, scope)?
            }
            (Expr::CVar(x), Expr::CVar(y))
            | (Expr::CvErr(x), Expr::CvErr(y))
            | (Expr::Eval(x), Expr::Eval(y))
            | (Expr::GuidFromString(x), Expr::GuidFromString(y))
            | (Expr::ErrorMessage(x), Expr::ErrorMessage(y))
            | (Expr::StringFromGuid(x), Expr::StringFromGuid(y)) => {
                self.same_group_expression_at(x, y, depth + 1, scope)?
            }
            (Expr::Convert(a, x), Expr::Convert(b, y)) => {
                a == b && self.same_group_expression_at(x, y, depth + 1, scope)?
            }
            (Expr::DateConvert(a, x), Expr::DateConvert(b, y)) => {
                a == b && self.same_group_expression_at(x, y, depth + 1, scope)?
            }
            (Expr::Inspect(a, x), Expr::Inspect(b, y)) => {
                a == b && self.same_group_expression_at(x, y, depth + 1, scope)?
            }
            (Expr::DateCall(a, x), Expr::DateCall(b, y)) => {
                a == b && self.same_group_expression_at(x, y, depth + 1, scope)?
            }
            (Expr::Binary(a, x, y), Expr::Binary(b, u, v)) => {
                a == b
                    && self.same_group_expression_at(x, u, depth + 1, scope)?
                    && self.same_group_expression_at(y, v, depth + 1, scope)?
            }
            (Expr::Like(x, y, a), Expr::Like(u, v, b)) => {
                a == b
                    && self.same_group_expression_at(x, u, depth + 1, scope)?
                    && self.same_group_expression_at(y, v, depth + 1, scope)?
            }
            (Expr::IIf(a, b, c), Expr::IIf(x, y, z))
            | (Expr::DateAdd(a, b, c), Expr::DateAdd(x, y, z)) => {
                self.same_group_expression_at(a, x, depth + 1, scope)?
                    && self.same_group_expression_at(b, y, depth + 1, scope)?
                    && self.same_group_expression_at(c, z, depth + 1, scope)?
            }
            (Expr::Between(a, b, c, flag), Expr::Between(x, y, z, other)) => {
                flag == other
                    && self.same_group_expression_at(a, x, depth + 1, scope)?
                    && self.same_group_expression_at(b, y, depth + 1, scope)?
                    && self.same_group_expression_at(c, z, depth + 1, scope)?
            }
            (Expr::DomainCall(a, args), Expr::DomainCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, scope)?
            }
            (Expr::ConditionalCall(a, args), Expr::ConditionalCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, scope)?
            }
            (Expr::CashFlowCall(a, args, array), Expr::CashFlowCall(b, other, other_array)) => {
                a == b
                    && array == other_array
                    && self.same_group_list(args, other, depth + 1, scope)?
            }
            (Expr::Array(a), Expr::Array(b)) => self.same_group_list(a, b, depth + 1, scope)?,
            (Expr::ValueInspect(a, value), Expr::ValueInspect(b, other)) => {
                a == b && self.same_group_expression_at(value, other, depth + 1, scope)?
            }
            (
                Expr::ArrayBound(a, input, dimension),
                Expr::ArrayBound(b, other, other_dimension),
            ) => {
                a == b
                    && self.same_group_expression_at(input, other, depth + 1, scope)?
                    && self.same_group_expression_at(
                        dimension,
                        other_dimension,
                        depth + 1,
                        scope,
                    )?
            }
            (Expr::FinancialCall(a, args), Expr::FinancialCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, scope)?
            }
            (Expr::MathCall(a, args), Expr::MathCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, scope)?
            }
            (Expr::ConstructorCall(a, args), Expr::ConstructorCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, scope)?
            }
            (Expr::TextCall(a, args), Expr::TextCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, scope)?
            }
            (Expr::InList(a, list, flag), Expr::InList(b, other, second)) => {
                flag == second
                    && self.same_group_expression_at(a, b, depth + 1, scope)?
                    && self.same_group_list(list, other, depth + 1, scope)?
            }
            (Expr::Nz(a, b), Expr::Nz(x, y)) | (Expr::Weekday(a, b), Expr::Weekday(x, y)) => {
                self.same_group_expression_at(a, x, depth + 1, scope)?
                    && self.same_group_option(b.as_deref(), y.as_deref(), depth + 1, scope)?
            }
            (Expr::DatePart(a, b, options), Expr::DatePart(x, y, other)) => {
                self.same_group_expression_at(a, x, depth + 1, scope)?
                    && self.same_group_expression_at(b, y, depth + 1, scope)?
                    && self.same_group_options(options, other, depth + 1, scope)?
            }
            (Expr::DateDiff(a, b, c, options), Expr::DateDiff(x, y, z, other)) => {
                self.same_group_expression_at(a, x, depth + 1, scope)?
                    && self.same_group_expression_at(b, y, depth + 1, scope)?
                    && self.same_group_expression_at(c, z, depth + 1, scope)?
                    && self.same_group_options(options, other, depth + 1, scope)?
            }
            // Subqueries and aggregates cannot be grouping keys. Their semantic
            // equivalence is not established by a syntactic comparison.
            _ => false,
        })
    }
    fn same_group_list(
        &mut self,
        left: &[Expr],
        right: &[Expr],
        depth: usize,
        scope: usize,
    ) -> Result<bool, QueryError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (a, b) in left.iter().zip(right) {
            if !self.same_group_expression_at(a, b, depth + 1, scope)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn same_group_option(
        &mut self,
        left: Option<&Expr>,
        right: Option<&Expr>,
        depth: usize,
        scope: usize,
    ) -> Result<bool, QueryError> {
        match (left, right) {
            (Some(a), Some(b)) => self.same_group_expression_at(a, b, depth + 1, scope),
            (None, None) => Ok(true),
            _ => Ok(false),
        }
    }
    fn same_group_options(
        &mut self,
        left: &[Option<Box<Expr>>; 2],
        right: &[Option<Box<Expr>>; 2],
        depth: usize,
        scope: usize,
    ) -> Result<bool, QueryError> {
        for (a, b) in left.iter().zip(right) {
            if !self.same_group_option(a.as_deref(), b.as_deref(), depth + 1, scope)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
