//! Bounded static provenance; never evaluates expressions, providers or records.
use super::Binder;
use crate::{
    QueryError,
    plan::{
        Expr, Slot,
        declarations::{combined, functions},
    },
};
use tablecraft_expressions::{functions::conversions::Conversion, numeric::OperandDeclaration};

impl Binder<'_> {
    pub(super) fn declaration_vector(
        &mut self,
        width: usize,
    ) -> Result<Vec<Option<OperandDeclaration>>, QueryError> {
        self.charge(
            width
                .checked_mul(size_of::<Option<OperandDeclaration>>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut declarations = Vec::new();
        declarations
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        Ok(declarations)
    }

    pub(super) fn unknown_declarations(
        &mut self,
        width: usize,
    ) -> Result<Vec<Option<OperandDeclaration>>, QueryError> {
        let mut declarations = self.declaration_vector(width)?;
        declarations.resize(width, None);
        Ok(declarations)
    }

    pub(super) fn result_declaration(
        &mut self,
        expression: &Expr,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<Option<OperandDeclaration>, QueryError> {
        self.tick(depth)?;
        Ok(match expression {
            Expr::Field { depth, index, .. } => {
                scopes
                    .get(*depth)
                    .and_then(|scope| scope.iter().filter(|slot| slot.available).nth(*index))
                    .ok_or(QueryError::Source)?
                    .declaration
            }
            Expr::Literal(_)
            | Expr::DateLiteral(..)
            | Expr::CollectionItem { .. }
            | Expr::ParentPresence { .. } => Some(OperandDeclaration::Scalar),
            Expr::Parameter(index) => Some(
                self.parameters
                    .get(*index)
                    .ok_or(QueryError::Source)?
                    .data_type
                    .declaration(),
            ),
            Expr::Convert(Conversion::Decimal, _)
            | Expr::CVar(_)
            | Expr::CvErr(_)
            | Expr::IIf(..)
            | Expr::Nz(..)
            | Expr::ConditionalCall(..) => Some(OperandDeclaration::Variant),
            Expr::Convert(_, _) => Some(OperandDeclaration::Scalar),
            Expr::Unary(_, value) => self.result_declaration(value, scopes, depth + 1)?,
            Expr::Binary(_, left, right) | Expr::Like(left, right, _) => combined(
                self.result_declaration(left, scopes, depth + 1)?,
                self.result_declaration(right, scopes, depth + 1)?,
            ),
            Expr::Between(value, first, second, _) => {
                let value = self.result_declaration(value, scopes, depth + 1)?;
                let first = self.result_declaration(first, scopes, depth + 1)?;
                let second = self.result_declaration(second, scopes, depth + 1)?;
                combined(combined(value, first), second)
            }
            Expr::InList(value, items, _) => {
                let mut declaration = self.result_declaration(value, scopes, depth + 1)?;
                for item in items {
                    declaration = combined(
                        declaration,
                        self.result_declaration(item, scopes, depth + 1)?,
                    );
                }
                declaration
            }
            Expr::Subquery(plan) => *plan.declarations.first().ok_or(QueryError::Width)?,
            Expr::InQuery(value, plan, _) | Expr::Quantified(value, _, _, plan) => combined(
                self.result_declaration(value, scopes, depth + 1)?,
                *plan.declarations.first().ok_or(QueryError::Width)?,
            ),
            _ => functions::result(expression),
        })
    }
}
