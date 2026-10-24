use super::Pruner;
use crate::{QueryError, plan::ParentPredicate};

impl Pruner<'_, '_> {
    pub(super) fn collect_parent(
        &mut self,
        predicate: &ParentPredicate,
        target: usize,
        offset: usize,
        needed: &mut [bool],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match predicate {
            ParentPredicate::Empty { .. } => {}
            ParentPredicate::Scalar(expr)
            | ParentPredicate::Any {
                predicate: expr, ..
            } => self.collect_expr(expr, target, offset, needed)?,
            ParentPredicate::AnyMany(product) => {
                self.collect_expr(&product.predicate, target, offset, needed)?
            }
            ParentPredicate::Not(value) => self.collect_parent(value, target, offset, needed)?,
            ParentPredicate::Logical(_, a, b) => {
                self.collect_parent(a, target, offset, needed)?;
                self.collect_parent(b, target, offset, needed)?;
            }
        }
        Ok(())
    }
    pub(super) fn rewrite_parent(
        &mut self,
        predicate: &mut ParentPredicate,
        scopes: &[&[Option<usize>]],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match predicate {
            ParentPredicate::Empty { .. } => {}
            ParentPredicate::Scalar(expr)
            | ParentPredicate::Any {
                predicate: expr, ..
            } => self.expression(expr, scopes)?,
            ParentPredicate::AnyMany(product) => self.expression(&mut product.predicate, scopes)?,
            ParentPredicate::Not(value) => self.rewrite_parent(value, scopes)?,
            ParentPredicate::Logical(_, a, b) => {
                self.rewrite_parent(a, scopes)?;
                self.rewrite_parent(b, scopes)?;
            }
        }
        Ok(())
    }
}
