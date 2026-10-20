//! Group membership uses bound expression structure, never SQL algebraic rewrites.
use super::Binder;
use crate::{QueryError, plan::Expr};

impl Binder<'_> {
    pub(super) fn validate_group(
        &mut self,
        expr: &Expr,
        keys: &[Expr],
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        for key in keys {
            if self.same_group_expression(expr, key, depth + 1)? {
                return Ok(());
            }
        }
        match expr {
            Expr::Field { depth: 0, .. } => return Err(QueryError::Grouping),
            Expr::Unary(_, e) | Expr::IsNull(e, _) | Expr::DateCall(_, e) | Expr::Convert(_, e) => {
                self.validate_group(e, keys, depth + 1)?
            }
            Expr::Binary(_, a, b) | Expr::Like(a, b, _) => {
                self.validate_group(a, keys, depth + 1)?;
                self.validate_group(b, keys, depth + 1)?;
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                self.validate_group(a, keys, depth + 1)?;
                self.validate_group(b, keys, depth + 1)?;
                self.validate_group(c, keys, depth + 1)?;
            }
            Expr::DateDiff(a, b, c, options) => {
                for expr in [a, b, c].into_iter().chain(options.iter().flatten()) {
                    self.validate_group(expr, keys, depth + 1)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for expr in [a, b].into_iter().chain(options.iter().flatten()) {
                    self.validate_group(expr, keys, depth + 1)?;
                }
            }
            Expr::TextCall(_, arguments)
            | Expr::MathCall(_, arguments)
            | Expr::ConstructorCall(_, arguments)
            | Expr::ConditionalCall(_, arguments) => {
                for expr in arguments {
                    self.validate_group(expr, keys, depth + 1)?;
                }
            }
            Expr::Nz(value, replacement) => {
                self.validate_group(value, keys, depth + 1)?;
                if let Some(expr) = replacement {
                    self.validate_group(expr, keys, depth + 1)?;
                }
            }
            Expr::InList(e, list, _) => {
                self.validate_group(e, keys, depth + 1)?;
                for e in list {
                    self.validate_group(e, keys, depth + 1)?;
                }
            }
            Expr::Subquery(_) | Expr::Exists(_) | Expr::InQuery(..) | Expr::Quantified(..) => {
                return Err(QueryError::Unsupported);
            }
            _ => {}
        }
        Ok(())
    }
}

impl Binder<'_> {
    pub(super) fn same_group_expression(
        &mut self,
        left: &Expr,
        right: &Expr,
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        Ok(match (left, right) {
            (Expr::Literal(a), Expr::Literal(b)) => a == b,
            (Expr::Field { depth: a, index: i }, Expr::Field { depth: b, index: j }) => {
                a == b && i == j
            }
            (Expr::Parameter(a), Expr::Parameter(b)) | (Expr::Output(a), Expr::Output(b)) => a == b,
            (Expr::Unary(a, x), Expr::Unary(b, y)) => {
                a == b && self.same_group_expression(x, y, depth + 1)?
            }
            (Expr::IsNull(x, a), Expr::IsNull(y, b)) => {
                a == b && self.same_group_expression(x, y, depth + 1)?
            }
            (Expr::Convert(a, x), Expr::Convert(b, y)) => {
                a == b && self.same_group_expression(x, y, depth + 1)?
            }
            (Expr::DateCall(a, x), Expr::DateCall(b, y)) => {
                a == b && self.same_group_expression(x, y, depth + 1)?
            }
            (Expr::Binary(a, x, y), Expr::Binary(b, u, v)) => {
                a == b
                    && self.same_group_expression(x, u, depth + 1)?
                    && self.same_group_expression(y, v, depth + 1)?
            }
            (Expr::Like(x, y, a), Expr::Like(u, v, b)) => {
                a == b
                    && self.same_group_expression(x, u, depth + 1)?
                    && self.same_group_expression(y, v, depth + 1)?
            }
            (Expr::IIf(a, b, c), Expr::IIf(x, y, z))
            | (Expr::DateAdd(a, b, c), Expr::DateAdd(x, y, z)) => {
                self.same_group_expression(a, x, depth + 1)?
                    && self.same_group_expression(b, y, depth + 1)?
                    && self.same_group_expression(c, z, depth + 1)?
            }
            (Expr::Between(a, b, c, flag), Expr::Between(x, y, z, other)) => {
                flag == other
                    && self.same_group_expression(a, x, depth + 1)?
                    && self.same_group_expression(b, y, depth + 1)?
                    && self.same_group_expression(c, z, depth + 1)?
            }
            (Expr::ConditionalCall(a, args), Expr::ConditionalCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1)?
            }
            (Expr::MathCall(a, args), Expr::MathCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1)?
            }
            (Expr::ConstructorCall(a, args), Expr::ConstructorCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1)?
            }
            (Expr::TextCall(a, args), Expr::TextCall(b, other)) => {
                a == b && self.same_group_list(args, other, depth + 1)?
            }
            (Expr::InList(a, list, flag), Expr::InList(b, other, second)) => {
                flag == second
                    && self.same_group_expression(a, b, depth + 1)?
                    && self.same_group_list(list, other, depth + 1)?
            }
            (Expr::Nz(a, b), Expr::Nz(x, y)) => {
                self.same_group_expression(a, x, depth + 1)?
                    && self.same_group_option(b.as_deref(), y.as_deref(), depth + 1)?
            }
            (Expr::DatePart(a, b, options), Expr::DatePart(x, y, other)) => {
                self.same_group_expression(a, x, depth + 1)?
                    && self.same_group_expression(b, y, depth + 1)?
                    && self.same_group_options(options, other, depth + 1)?
            }
            (Expr::DateDiff(a, b, c, options), Expr::DateDiff(x, y, z, other)) => {
                self.same_group_expression(a, x, depth + 1)?
                    && self.same_group_expression(b, y, depth + 1)?
                    && self.same_group_expression(c, z, depth + 1)?
                    && self.same_group_options(options, other, depth + 1)?
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
    ) -> Result<bool, QueryError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (a, b) in left.iter().zip(right) {
            if !self.same_group_expression(a, b, depth + 1)? {
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
    ) -> Result<bool, QueryError> {
        match (left, right) {
            (Some(a), Some(b)) => self.same_group_expression(a, b, depth + 1),
            (None, None) => Ok(true),
            _ => Ok(false),
        }
    }
    fn same_group_options(
        &mut self,
        left: &[Option<Box<Expr>>; 2],
        right: &[Option<Box<Expr>>; 2],
        depth: usize,
    ) -> Result<bool, QueryError> {
        for (a, b) in left.iter().zip(right) {
            if !self.same_group_option(a.as_deref(), b.as_deref(), depth + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
