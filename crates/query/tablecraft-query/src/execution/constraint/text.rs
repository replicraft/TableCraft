use super::Runtime;
use crate::execution::collation::predicate_error;
use crate::{
    QueryError,
    plan::Expr,
    source::{ScanConstraint, TextPolicy},
};
use tablecraft_core::values::Value;
use tablecraft_sql::select::Expression;

impl Runtime<'_, '_> {
    pub(super) fn text_equality_constraint(
        &mut self,
        column: usize,
        field: &Expr,
        constant: &Expr,
    ) -> Result<Option<ScanConstraint>, QueryError> {
        let Some(resolver) = self.options.collation_resolver else {
            return Ok(None);
        };
        let TextPolicy::Column(policy) = self.text_policy(field)? else {
            return Ok(None);
        };
        self.tick()?;
        let literal = match constant {
            Expr::Literal(Expression::Text(value)) => value.as_str(),
            Expr::Parameter(index) => match self.parameters.get(*index).map(|value| value.value) {
                Some(Value::Text(value)) => value.as_str(),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        if literal.len() > self.options.text_limit() {
            return Ok(None);
        }
        let _span = tracing::debug_span!("query.constraint.text_equality").entered();
        let key = resolver
            .equality_key(policy, literal)
            .map_err(|error| match error {
                QueryError::Evaluation(error) => predicate_error(error),
                error => error,
            })?;
        self.tick()?;
        let Some(key) = key else {
            return Ok(None);
        };
        if key.policy() != policy {
            return Err(QueryError::Source);
        }
        Ok(Some(ScanConstraint::TextEqual { column, key }))
    }
}
