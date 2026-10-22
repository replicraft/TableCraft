//! Structural identities for grouping and projected sort keys.
use super::Binder;
use crate::plan::identity::{Comparer, Identity};
use crate::{QueryError, plan::Expr, source::TextPolicy};

impl Binder<'_> {
    pub(super) fn same_group_expression_at(
        &mut self,
        left: &Expr,
        right: &Expr,
        depth: usize,
        shift: usize,
    ) -> Result<bool, QueryError> {
        Comparer {
            tick: &mut |depth| self.tick(depth),
        }
        .same_expression(
            left,
            right,
            depth,
            Identity {
                shift,
                projection: false,
            },
        )
    }

    fn same_projected_expression(
        &mut self,
        left: &Expr,
        right: &Expr,
        depth: usize,
    ) -> Result<bool, QueryError> {
        Comparer {
            tick: &mut |depth| self.tick(depth),
        }
        .same_expression(
            left,
            right,
            depth,
            Identity {
                shift: 0,
                projection: true,
            },
        )
    }
}

impl Binder<'_> {
    pub(super) fn projected_sort(
        &mut self,
        sort: &mut [(Expr, bool)],
        projection: &[Expr],
        policies: &[TextPolicy],
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        for (expression, _) in sort {
            if matches!(expression, Expr::Output(..)) {
                continue;
            }
            for (index, projected) in projection.iter().enumerate() {
                if self.same_projected_expression(expression, projected, depth + 1)? {
                    *expression = Expr::Output(index, policies[index]);
                    break;
                }
            }
        }
        Ok(())
    }
}
