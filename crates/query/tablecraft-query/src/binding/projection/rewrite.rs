use super::Pruner;
use crate::{QueryError, plan::Expr};

impl Pruner<'_, '_> {
    pub(super) fn expression(
        &mut self,
        expr: &mut Expr,
        scopes: &[&[Option<usize>]],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match expr {
            Expr::Field { depth, index } => {
                *index = scopes
                    .get(*depth)
                    .and_then(|map| map.get(*index))
                    .copied()
                    .flatten()
                    .ok_or(QueryError::Source)?;
            }
            Expr::Unary(_, expr) | Expr::IsNull(expr, _) | Expr::DateCall(_, expr) => {
                self.expression(expr, scopes)?
            }
            Expr::Binary(_, a, b) | Expr::Like(a, b, _) => {
                self.expression(a, scopes)?;
                self.expression(b, scopes)?;
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                for expr in [a, b, c] {
                    self.expression(expr, scopes)?;
                }
            }
            Expr::DateDiff(a, b, c, options) => {
                for expr in [a, b, c].into_iter().chain(options.iter_mut().flatten()) {
                    self.expression(expr, scopes)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for expr in [a, b].into_iter().chain(options.iter_mut().flatten()) {
                    self.expression(expr, scopes)?;
                }
            }
            Expr::TextCall(_, arguments) => {
                for expr in arguments {
                    self.expression(expr, scopes)?;
                }
            }
            Expr::Nz(value, replacement) => {
                self.expression(value, scopes)?;
                if let Some(expr) = replacement {
                    self.expression(expr, scopes)?;
                }
            }
            Expr::InList(value, list, _) => {
                self.expression(value, scopes)?;
                for expr in list {
                    self.expression(expr, scopes)?;
                }
            }
            Expr::Aggregate(_, Some(expr)) => self.expression(expr, scopes)?,
            Expr::Subquery(read) | Expr::Exists(read) => self.read(read, scopes)?,
            Expr::InQuery(expr, read, _) | Expr::Quantified(expr, _, _, read) => {
                self.expression(expr, scopes)?;
                self.read(read, scopes)?;
            }
            Expr::Literal(_) | Expr::Output(_) | Expr::Parameter(_) | Expr::Aggregate(_, None) => {}
        }
        Ok(())
    }
}
