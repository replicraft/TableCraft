//! Bind array intermediates only at explicit consuming functions.
use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
};
use tablecraft_expressions::arrays::{BoundFunction, sql_arguments};
use tablecraft_sql::select::{Expression, FunctionArgument};

impl Binder<'_> {
    pub(super) fn inspection_operand(
        &mut self,
        expression: &Expression,
        scopes: &[&[Slot]],
        aggregates: bool,
        depth: usize,
    ) -> Result<Expr, QueryError> {
        if let Expression::Call {
            qualifier,
            name,
            arguments,
        } = expression
            && sql_arguments::is_constructor(qualifier, name)
        {
            self.tick(depth)?;
            let elements = sql_arguments::elements(arguments).ok_or(QueryError::Unsupported)?;
            self.charge(elements.len() * size_of::<Expr>())?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(elements.len())
                .map_err(|_| QueryError::Allocation)?;

            for argument in elements {
                let FunctionArgument::Expression(element) = argument else {
                    return Err(QueryError::Unsupported);
                };
                values.push(self.expression(element, scopes, aggregates, depth + 1)?);
            }

            return Ok(Expr::Array(values));
        }

        self.expression(expression, scopes, aggregates, depth)
    }

    pub(super) fn array_bound(
        &mut self,
        function: BoundFunction,
        arguments: &[FunctionArgument],
        scopes: &[&[Slot]],
        aggregates: bool,
        depth: usize,
    ) -> Result<Expr, QueryError> {
        let [FunctionArgument::Expression(input), options @ ..] = arguments else {
            return Err(QueryError::Unsupported);
        };
        if options.len() > 1
            || options.iter().any(|arg| {
                !matches!(
                    arg,
                    FunctionArgument::Expression(_) | FunctionArgument::Missing
                )
            })
        {
            return Err(QueryError::Unsupported);
        }

        let input = self.inspection_operand(input, scopes, aggregates, depth + 1)?;
        let dimension = match options.first() {
            Some(FunctionArgument::Expression(expression)) => {
                self.expression(expression, scopes, aggregates, depth + 1)?
            }
            _ => Expr::Literal(Expression::Integer(1)),
        };

        Ok(Expr::ArrayBound(
            function,
            Box::new(input),
            Box::new(dimension),
        ))
    }
}
