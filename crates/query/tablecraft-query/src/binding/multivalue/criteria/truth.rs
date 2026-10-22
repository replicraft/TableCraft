//! Distinguish independent Boolean memberships from per-item integer operators.
use super::{Binder, Expr, QueryError, Relation, referenced_fields};
use tablecraft_expressions::functions::{
    conversions::Conversion, inspection::types::ValueInspectionFunction,
};
use tablecraft_sql::select::{BinaryOperator as B, Expression, UnaryOperator as U};

impl Binder<'_> {
    pub(super) fn foldable_truth(
        &mut self,
        expr: &Expr,
        source: &Relation,
        fields: &[(usize, usize)],
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Unary(U::Not, value) => {
                return self.foldable_truth(value, source, fields, depth + 1);
            }
            Expr::Binary(
                B::And | B::Or | B::Xor | B::Equivalence | B::Implication,
                left,
                right,
            ) => {
                let left = self.foldable_truth(left, source, fields, depth + 1)?;
                let right = self.foldable_truth(right, source, fields, depth + 1)?;
                return Ok(left && right);
            }
            _ => {}
        }
        if self.boolean_result(expr, depth)? {
            return Ok(true);
        }
        let used = referenced_fields(std::slice::from_ref(expr), source.row_width(), self)?;
        Ok(!fields.iter().any(|(field, _)| used[*field]))
    }

    fn boolean_result(&mut self, expr: &Expr, depth: usize) -> Result<bool, QueryError> {
        self.tick(depth)?;
        Ok(match expr {
            Expr::Literal(Expression::Boolean(_) | Expression::Null)
            | Expr::IsNull(..)
            | Expr::Inspect(..)
            | Expr::ValueInspect(
                ValueInspectionFunction::IsArray
                | ValueInspectionFunction::IsEmpty
                | ValueInspectionFunction::IsError,
                _,
            )
            | Expr::Convert(Conversion::Boolean, _)
            | Expr::Like(..)
            | Expr::Between(..)
            | Expr::InList(..)
            | Expr::InQuery(..)
            | Expr::Exists(..)
            | Expr::Quantified(..) => true,
            Expr::Binary(
                B::Equal | B::NotEqual | B::Less | B::LessOrEqual | B::Greater | B::GreaterOrEqual,
                _,
                _,
            ) => true,
            Expr::Unary(U::Not, value) | Expr::CVar(value) => {
                self.boolean_result(value, depth + 1)?
            }
            Expr::Binary(
                B::And | B::Or | B::Xor | B::Equivalence | B::Implication,
                left,
                right,
            ) => {
                let left = self.boolean_result(left, depth + 1)?;
                let right = self.boolean_result(right, depth + 1)?;
                left && right
            }
            Expr::IIf(_, yes, no) => {
                let yes = self.boolean_result(yes, depth + 1)?;
                let no = self.boolean_result(no, depth + 1)?;
                yes && no
            }
            _ => false,
        })
    }
}
