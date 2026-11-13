//! Bounded formatting of changed owned leaves, using the SQL syntax owner.
use super::{SavedReadError, context::Context};
use std::fmt::{self, Display, Write};
use tablecraft_core::limits::Resource;
use tablecraft_sql::select::{Expression, FunctionArgument, Projection};

const TEXT_ESCAPE_COPIES: u64 = 2;

#[derive(Debug)]
pub(super) enum Error {
    Metadata(SavedReadError),
    Unsupported,
    Roundtrip,
}

impl From<SavedReadError> for Error {
    fn from(error: SavedReadError) -> Self {
        Self::Metadata(error)
    }
}

struct Output<'a, 'b> {
    context: &'a mut Context<'b>,
    text: String,
    error: Option<SavedReadError>,
}

impl Write for Output<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        let result = (|| {
            self.context.syntax(text.len())?;
            let length = self
                .text
                .len()
                .checked_add(text.len())
                .filter(|length| *length <= self.context.limits.max_bytes)
                .ok_or(SavedReadError::SyntaxLimit)?;
            self.context
                .budget
                .charge(Resource::Steps, text.len() as u64)?;
            if length > self.text.capacity() {
                // Pay for the full new allocation and copying existing content,
                // not merely the additional logical text bytes.
                self.context.budget.charge_allocation(length as u64, 1)?;
                self.context
                    .budget
                    .charge(Resource::Steps, self.text.len() as u64)?;
                self.text
                    .try_reserve_exact(text.len())
                    .map_err(|_| SavedReadError::Allocation)?;
            }
            self.text.push_str(text);
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error);
            return Err(fmt::Error);
        }
        Ok(())
    }
}

fn text(value: &impl Display, context: &mut Context<'_>) -> Result<String, Error> {
    let mut output = Output {
        context,
        text: String::new(),
        error: None,
    };
    let formatted = write!(output, "{value}");
    if let Some(error) = output.error {
        return Err(error.into());
    }
    formatted.map_err(|_| Error::Roundtrip)?;
    Ok(output.text)
}

pub(super) fn projection(value: &Projection, context: &mut Context<'_>) -> Result<String, Error> {
    validate_projection(value, context)?;
    let formatted = match value {
        Projection::Expression { expression, .. } => {
            escape_allowance(expression, context)?;
            text(expression, context)?
        }
        Projection::Field { field, .. } => text(field, context)?,
        Projection::Wildcard(_) => return Err(Error::Unsupported),
    };
    let parsed = context.projection(&formatted)?;
    if !same_projection_body(value, &parsed) {
        return Err(Error::Roundtrip);
    }
    Ok(formatted)
}

pub(super) fn expression(value: &Expression, context: &mut Context<'_>) -> Result<String, Error> {
    validate_expression(value, context)?;
    escape_allowance(value, context)?;
    let formatted = text(value, context)?;
    if context.expression(&formatted)? != *value {
        return Err(Error::Roundtrip);
    }
    Ok(formatted)
}

pub(super) fn same_projection_body(left: &Projection, right: &Projection) -> bool {
    match (left, right) {
        (
            Projection::Expression {
                expression: left, ..
            },
            Projection::Expression {
                expression: right, ..
            },
        ) => left == right,
        (Projection::Field { field: left, .. }, Projection::Field { field: right, .. }) => {
            left == right
        }
        (
            Projection::Field { field, .. },
            Projection::Expression {
                expression: Expression::Field(other),
                ..
            },
        )
        | (
            Projection::Expression {
                expression: Expression::Field(other),
                ..
            },
            Projection::Field { field, .. },
        ) => field == other,
        (Projection::Wildcard(left), Projection::Wildcard(right)) => left == right,
        _ => false,
    }
}

pub(super) fn validate_projection(
    value: &Projection,
    context: &mut Context<'_>,
) -> Result<(), Error> {
    match value {
        Projection::Expression { expression, .. } => validate_expression(expression, context),
        Projection::Field { .. } => {
            context.check()?;
            Ok(())
        }
        Projection::Wildcard(_) => Err(Error::Unsupported),
    }
}

pub(super) fn validate_expression(
    value: &Expression,
    context: &mut Context<'_>,
) -> Result<(), Error> {
    walk(value, context, &mut |_, _| Ok(()))
}

fn escape_allowance(value: &Expression, context: &mut Context<'_>) -> Result<(), Error> {
    // Display owns SQL quoting. Its temporary escaped text must be admitted
    // before formatting starts, including on a zero-allocation allowance.
    walk(value, context, &mut |expression, context| {
        if let Expression::Text(value) = expression {
            context
                .budget
                .charge_allocation(value.len() as u64, TEXT_ESCAPE_COPIES)
                .map_err(SavedReadError::from)?;
        }
        Ok(())
    })
}

fn walk(
    value: &Expression,
    context: &mut Context<'_>,
    visitor: &mut impl FnMut(&Expression, &mut Context<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    context.check()?;
    visitor(value, context)?;
    match value {
        Expression::Subquery(_)
        | Expression::Exists(_)
        | Expression::InQuery { .. }
        | Expression::Quantified { .. } => return Err(Error::Unsupported),
        Expression::Unary { expression, .. }
        | Expression::Member { expression, .. }
        | Expression::IsNull { expression, .. } => walk(expression, context, visitor)?,
        Expression::Binary { left, right, .. } => {
            walk(left, context, visitor)?;
            walk(right, context, visitor)?;
        }
        Expression::Like {
            expression,
            pattern,
            ..
        } => {
            walk(expression, context, visitor)?;
            walk(pattern, context, visitor)?;
        }
        Expression::Between {
            expression,
            first_bound,
            second_bound,
            ..
        } => {
            walk(expression, context, visitor)?;
            walk(first_bound, context, visitor)?;
            walk(second_bound, context, visitor)?;
        }
        Expression::InList {
            expression, values, ..
        } => {
            walk(expression, context, visitor)?;
            for value in values {
                walk(value, context, visitor)?;
            }
        }
        Expression::Call { arguments, .. } => {
            for argument in arguments {
                if let FunctionArgument::Expression(expression) = argument {
                    walk(expression, context, visitor)?;
                }
            }
        }
        Expression::Field(_)
        | Expression::Integer(_)
        | Expression::Text(_)
        | Expression::Number(_)
        | Expression::DateLiteral(_)
        | Expression::GuidLiteral(_)
        | Expression::Identity
        | Expression::Null
        | Expression::Boolean(_) => {}
    }
    Ok(())
}
