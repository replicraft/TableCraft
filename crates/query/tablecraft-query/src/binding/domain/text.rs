//! Bounded inert expression parsing under the caller's cumulative query budget.
use crate::{QueryError, profile::MAX_BIND_DEPTH};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    ParseError, ParseErrorKind, ParseLimits, parse_expression, parse_select,
    select::{Expression, FunctionArgument, Select},
};

const AST_TEXT_COPIES: u64 = 2;

pub(super) struct Text<'a> {
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
            max_depth: MAX_BIND_DEPTH,
        })
    }
    fn join(&mut self, parts: &[&str]) -> Result<String, QueryError> {
        self.check()?;
        let bytes = parts
            .iter()
            .try_fold(0_usize, |n, s| n.checked_add(s.len()))
            .filter(|n| *n <= ParseLimits::MAX_BYTES)
            .ok_or(QueryError::StructureLimit)?;
        self.budget.charge_allocation(bytes as u64, 1)?;
        let mut text = String::new();
        text.try_reserve_exact(bytes)
            .map_err(|_| QueryError::Allocation)?;
        for part in parts {
            text.push_str(part);
        }
        Ok(text)
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
        let text = self.join(&["SELECT 0 FROM [", name, "]"])?;
        let limits = self.limits(text.len())?;
        let result = parse_select(&text, limits).map_err(adapt)?.into_query();
        self.check()?;
        Ok(result)
    }
    pub fn expression(&mut self, text: &str) -> Result<Expression, QueryError> {
        let limits = self.limits(text.len())?;
        let original = parse_expression(text, limits).map_err(adapt)?;
        let guarded = self.join(&["(", text, "\n)"])?;
        let limits = self.limits(guarded.len())?;
        let parsed = parse_expression(&guarded, limits).map_err(adapt)?;
        self.check()?;
        if original != parsed {
            return Err(QueryError::Unsupported);
        }
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
