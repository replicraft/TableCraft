//! Bounded inert expression parsing under the caller's cumulative query budget.
use crate::{BindingDepth, QueryError};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    ParseError, ParseErrorKind, ParseLimits, parse_expression_strict,
    select::{Expression, FunctionArgument, Name, Projection, Select, Table, TableSource},
};

const AST_TEXT_COPIES: u64 = 2;

pub(super) struct Text<'a> {
    pub binding_depth: BindingDepth,
    pub budget: &'a mut ResourceBudget,
    pub cancelled: Option<&'a dyn Fn() -> bool>,
}
impl Text<'_> {
    fn check(&mut self) -> Result<(), QueryError> {
        if self.cancelled.is_some_and(|f| f()) {
            return Err(QueryError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }
    fn limits(&mut self, bytes: usize) -> Result<ParseLimits, QueryError> {
        self.check()?;
        if bytes > ParseLimits::MAX_BYTES {
            return Err(QueryError::StructureLimit);
        }
        let tokens = bytes.saturating_add(1).min(ParseLimits::MAX_TOKENS);
        self.budget
            .charge(Resource::Steps, (bytes + tokens) as u64)?;
        self.budget
            .charge_allocation(bytes as u64, AST_TEXT_COPIES)?;
        self.budget
            .charge_allocation(tokens as u64, size_of::<Expression>() as u64)?;
        Ok(ParseLimits {
            max_bytes: bytes,
            max_tokens: tokens,
            max_depth: self.binding_depth.get(),
        })
    }
    fn scalar_syntax(&mut self, expr: &Expression) -> Result<(), QueryError> {
        self.check()?;
        match expr {
            Expression::Subquery(_)
            | Expression::Exists(_)
            | Expression::InQuery { .. }
            | Expression::Quantified { .. }
            | Expression::Member { .. }
            | Expression::Identity => return Err(QueryError::Unsupported),
            Expression::Call { arguments, .. } => {
                for argument in arguments {
                    if let FunctionArgument::Expression(expr) = argument {
                        self.scalar_syntax(expr)?;
                    }
                }
            }
            Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
                self.scalar_syntax(expression)?
            }
            Expression::Binary { left, right, .. } => {
                self.scalar_syntax(left)?;
                self.scalar_syntax(right)?;
            }
            Expression::Like {
                expression,
                pattern,
                ..
            } => {
                self.scalar_syntax(expression)?;
                self.scalar_syntax(pattern)?;
            }
            Expression::Between {
                expression,
                first_bound,
                second_bound,
                ..
            } => {
                for expr in [expression, first_bound, second_bound] {
                    self.scalar_syntax(expr)?;
                }
            }
            Expression::InList {
                expression, values, ..
            } => {
                self.scalar_syntax(expression)?;
                for expr in values {
                    self.scalar_syntax(expr)?;
                }
            }
            Expression::Field(_)
            | Expression::Integer(_)
            | Expression::Text(_)
            | Expression::Number(_)
            | Expression::DateLiteral(_)
            | Expression::GuidLiteral(_)
            | Expression::Null
            | Expression::Boolean(_) => {}
        }
        Ok(())
    }
    pub fn source(&mut self, name: &str) -> Result<Select, QueryError> {
        if name.is_empty() || name.chars().any(|c| c.is_control() || "[]`.!".contains(c)) {
            return Err(QueryError::Unsupported);
        }

        // Preserve the old bounded carrier allowance, without building SQL text.
        self.check()?;
        let bytes = "SELECT 0 FROM []"
            .len()
            .checked_add(name.len())
            .filter(|bytes| *bytes <= ParseLimits::MAX_BYTES)
            .ok_or(QueryError::StructureLimit)?;
        self.budget.charge_allocation(bytes as u64, 1)?;

        let limits = self.limits(bytes)?;
        let name = Name::from_identifier(name, limits).map_err(adapt)?;
        let result = Select::from_projection(
            vec![Projection::Expression {
                expression: Expression::Integer(0),
                alias: None,
            }],
            vec![TableSource::Table(Table {
                database: None,
                name,
                alias: None,
            })],
        );

        self.check()?;
        Ok(result)
    }

    pub fn expression(&mut self, text: &str) -> Result<Expression, QueryError> {
        let limits = self.limits(text.len())?;
        let original = parse_expression_strict(text, limits).map_err(adapt)?;
        self.check()?;
        self.scalar_syntax(&original)?;
        Ok(original)
    }
}
fn adapt(error: ParseError) -> QueryError {
    match error.kind {
        ParseErrorKind::Limit => QueryError::StructureLimit,
        _ => QueryError::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_sql::parse_select;

    #[test]
    fn owned_domain_source_matches_syntax_and_keeps_name_guards() {
        let mut budget = ResourceBudget::new(1 << 24, 100, 1 << 20);
        let mut text = Text {
            binding_depth: BindingDepth::default(),
            budget: &mut budget,
            cancelled: None,
        };
        let expected = parse_select("SELECT 0 FROM [Order Details]", ParseLimits::default())
            .unwrap()
            .into_query();
        assert_eq!(text.source("Order Details").unwrap(), expected);

        for name in ["", "a.b", "a!b", "a`b", "a[b", "a]b", "a\nb"] {
            assert_eq!(text.source(name), Err(QueryError::Unsupported));
        }
    }

    #[test]
    fn owned_domain_source_keeps_cancellation_and_allocation_limits() {
        let mut budget = ResourceBudget::new(1 << 24, 100, 1 << 20);
        let mut text = Text {
            binding_depth: BindingDepth::default(),
            budget: &mut budget,
            cancelled: Some(&|| true),
        };
        assert_eq!(text.source("Items"), Err(QueryError::Cancelled));

        let mut budget = ResourceBudget::new(0, 100, 1 << 20);
        let mut text = Text {
            binding_depth: BindingDepth::default(),
            budget: &mut budget,
            cancelled: None,
        };
        assert!(text.source("Items").is_err());
    }
}
