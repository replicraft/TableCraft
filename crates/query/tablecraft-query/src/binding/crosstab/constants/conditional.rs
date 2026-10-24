//! Constant eager choices move their selected payload without another allocation.
use super::super::super::Binder;
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::{self, conditional::ConditionalFunction};

impl Binder<'_> {
    pub(super) fn constant_iif(
        &mut self,
        condition: &Expr,
        yes: &Expr,
        no: &Expr,
        depth: usize,
    ) -> Result<Value, QueryError> {
        let condition = self.constant_heading(condition, depth)?;
        let yes = self.constant_heading(yes, depth)?;
        let no = self.constant_heading(no, depth)?;

        // Text-to-Boolean conversion needs a runtime number policy. Both branches
        // are still visited: a selected constant cannot hide contextual operands.
        if matches!(condition, Value::Text(_)) {
            return Err(QueryError::Unsupported);
        }

        functions::iif(&condition, yes, no).map_err(QueryError::Evaluation)
    }

    pub(super) fn constant_nz(
        &mut self,
        input: &Expr,
        replacement: Option<&Expr>,
        depth: usize,
    ) -> Result<Value, QueryError> {
        let input = self.constant_heading(input, depth)?;
        let replacement = replacement
            .map(|value| self.constant_heading(value, depth))
            .transpose()?;

        Ok(functions::nz_query(input, replacement))
    }

    pub(super) fn constant_selection(
        &mut self,
        function: ConditionalFunction,
        arguments: &[Expr],
        depth: usize,
    ) -> Result<Value, QueryError> {
        let mut selection = function
            .start(arguments.len())
            .map_err(QueryError::Evaluation)?;

        for argument in arguments {
            let value = self.constant_heading(argument, depth)?;
            if selection.is_control() && matches!(value, Value::Text(_)) {
                return Err(QueryError::Unsupported);
            }

            // The shared selector defers control conversion errors until all
            // arguments arrive. Later eager expression errors retain precedence.
            selection.push(value).map_err(QueryError::Evaluation)?;
        }

        selection.finish().map_err(QueryError::Evaluation)
    }
}
