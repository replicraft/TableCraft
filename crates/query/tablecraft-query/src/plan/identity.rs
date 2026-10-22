//! Metered structural expression identity shared by binding and runtime provenance.
use crate::{QueryError, plan::Expr};
use tablecraft_expressions::functions::math::MathFunction;

#[derive(Clone, Copy)]
pub(crate) struct Identity {
    pub shift: usize,
    pub projection: bool,
}
pub(crate) struct Comparer<'a> {
    pub tick: &'a mut dyn FnMut(usize) -> Result<(), QueryError>,
}
impl Comparer<'_> {
    pub(crate) fn same_expression(
        &mut self,
        left: &Expr,
        right: &Expr,
        depth: usize,
        context: Identity,
    ) -> Result<bool, QueryError> {
        (self.tick)(depth)?;
        Ok(match (left, right) {
            (Expr::MathCall(MathFunction::Rnd, _), _) | (Expr::DomainCall(..), _)
                if context.projection =>
            {
                false
            }
            (Expr::Literal(a), Expr::Literal(b)) => a == b,
            (Expr::CurrentUser, Expr::CurrentUser) => true,
            (
                Expr::Field {
                    depth: a, index: i, ..
                },
                Expr::Field {
                    depth: b, index: j, ..
                },
            ) => *a == *b + context.shift && i == j,
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
            ) => *a == *b + context.shift && i == j && x == y,
            (Expr::Unary(a, x), Expr::Unary(b, y)) => {
                a == b && self.same_expression(x, y, depth + 1, context)?
            }
            (Expr::IsNull(x, a), Expr::IsNull(y, b)) => {
                a == b && self.same_expression(x, y, depth + 1, context)?
            }
            (Expr::CVar(x), Expr::CVar(y))
            | (Expr::CvErr(x), Expr::CvErr(y))
            | (Expr::Eval(x), Expr::Eval(y))
            | (Expr::GuidFromString(x), Expr::GuidFromString(y))
            | (Expr::ErrorMessage(x), Expr::ErrorMessage(y))
            | (Expr::StringFromGuid(x), Expr::StringFromGuid(y)) => {
                self.same_expression(x, y, depth + 1, context)?
            }
            (Expr::Convert(a, x), Expr::Convert(b, y)) => {
                a == b && self.same_expression(x, y, depth + 1, context)?
            }
            (Expr::DateConvert(a, x), Expr::DateConvert(b, y)) => {
                a == b && self.same_expression(x, y, depth + 1, context)?
            }
            (Expr::Inspect(a, x), Expr::Inspect(b, y)) => {
                a == b && self.same_expression(x, y, depth + 1, context)?
            }
            (Expr::DateCall(a, x), Expr::DateCall(b, y)) => {
                a == b && self.same_expression(x, y, depth + 1, context)?
            }
            (Expr::Binary(a, x, y), Expr::Binary(b, u, v)) => {
                a == b
                    && self.same_expression(x, u, depth + 1, context)?
                    && self.same_expression(y, v, depth + 1, context)?
            }
            (Expr::Like(x, y, a), Expr::Like(u, v, b)) => {
                a == b
                    && self.same_expression(x, u, depth + 1, context)?
                    && self.same_expression(y, v, depth + 1, context)?
            }
            (Expr::IIf(a, b, c), Expr::IIf(x, y, z))
            | (Expr::DateAdd(a, b, c), Expr::DateAdd(x, y, z)) => {
                self.same_expression(a, x, depth + 1, context)?
                    && self.same_expression(b, y, depth + 1, context)?
                    && self.same_expression(c, z, depth + 1, context)?
            }
            (Expr::Between(a, b, c, flag), Expr::Between(x, y, z, other)) => {
                flag == other
                    && self.same_expression(a, x, depth + 1, context)?
                    && self.same_expression(b, y, depth + 1, context)?
                    && self.same_expression(c, z, depth + 1, context)?
            }
            (Expr::DomainCall(a, args), Expr::DomainCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, context)?
            }
            (Expr::ConditionalCall(a, args), Expr::ConditionalCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, context)?
            }
            (Expr::CashFlowCall(a, args, array), Expr::CashFlowCall(b, other, other_array)) => {
                a == b
                    && array == other_array
                    && self.same_group_list(args, other, depth + 1, context)?
            }
            (Expr::Array(a), Expr::Array(b)) => self.same_group_list(a, b, depth + 1, context)?,
            (Expr::ValueInspect(a, value), Expr::ValueInspect(b, other)) => {
                a == b && self.same_expression(value, other, depth + 1, context)?
            }
            (
                Expr::ArrayBound(a, input, dimension),
                Expr::ArrayBound(b, other, other_dimension),
            ) => {
                a == b
                    && self.same_expression(input, other, depth + 1, context)?
                    && self.same_expression(dimension, other_dimension, depth + 1, context)?
            }
            (Expr::FinancialCall(a, args), Expr::FinancialCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, context)?
            }
            (Expr::MathCall(a, args), Expr::MathCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, context)?
            }
            (Expr::ConstructorCall(a, args), Expr::ConstructorCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, context)?
            }
            (Expr::TextCall(a, args), Expr::TextCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1, context)?
            }
            (Expr::InList(a, list, flag), Expr::InList(b, other, second)) => {
                flag == second
                    && self.same_expression(a, b, depth + 1, context)?
                    && self.same_group_list(list, other, depth + 1, context)?
            }
            (Expr::Nz(a, b), Expr::Nz(x, y)) | (Expr::Weekday(a, b), Expr::Weekday(x, y)) => {
                self.same_expression(a, x, depth + 1, context)?
                    && self.same_group_option(b.as_deref(), y.as_deref(), depth + 1, context)?
            }
            (Expr::DatePart(a, b, options), Expr::DatePart(x, y, other)) => {
                self.same_expression(a, x, depth + 1, context)?
                    && self.same_expression(b, y, depth + 1, context)?
                    && self.same_group_options(options, other, depth + 1, context)?
            }
            (Expr::DateDiff(a, b, c, options), Expr::DateDiff(x, y, z, other)) => {
                self.same_expression(a, x, depth + 1, context)?
                    && self.same_expression(b, y, depth + 1, context)?
                    && self.same_expression(c, z, depth + 1, context)?
                    && self.same_group_options(options, other, depth + 1, context)?
            }
            (Expr::Aggregate(a, left), Expr::Aggregate(b, right)) if context.projection => {
                a == b
                    && self.same_group_option(
                        left.as_deref(),
                        right.as_deref(),
                        depth + 1,
                        context,
                    )?
            }
            // Read plans and volatile calls have no reusable structural identity.
            _ => false,
        })
    }
    fn same_group_list(
        &mut self,
        left: &[Expr],
        right: &[Expr],
        depth: usize,
        context: Identity,
    ) -> Result<bool, QueryError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (a, b) in left.iter().zip(right) {
            if !self.same_expression(a, b, depth + 1, context)? {
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
        context: Identity,
    ) -> Result<bool, QueryError> {
        match (left, right) {
            (Some(a), Some(b)) => self.same_expression(a, b, depth + 1, context),
            (None, None) => Ok(true),
            _ => Ok(false),
        }
    }
    fn same_group_options(
        &mut self,
        left: &[Option<Box<Expr>>; 2],
        right: &[Option<Box<Expr>>; 2],
        depth: usize,
        context: Identity,
    ) -> Result<bool, QueryError> {
        for (a, b) in left.iter().zip(right) {
            if !self.same_group_option(a.as_deref(), b.as_deref(), depth + 1, context)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
