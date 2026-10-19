use super::SavedSelectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    ParseLimits, parse_expression, parse_query, parse_select,
    query::Query,
    select::{Expression, Name, Projection, Select},
};

const PARSED_TEXT_COPIES: u64 = 2;

pub(super) struct Context<'a> {
    pub budget: &'a mut ResourceBudget,
    pub cancelled: &'a dyn Fn() -> bool,
    pub limits: ParseLimits,
    syntax_bytes: usize,
}
impl<'a> Context<'a> {
    pub fn new(
        budget: &'a mut ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
        limits: ParseLimits,
    ) -> Self {
        Self {
            budget,
            cancelled,
            limits: ParseLimits {
                max_bytes: limits.max_bytes.min(ParseLimits::MAX_BYTES),
                max_tokens: limits.max_tokens.min(ParseLimits::MAX_TOKENS),
                max_depth: limits.max_depth.min(ParseLimits::MAX_DEPTH),
            },
            syntax_bytes: 0,
        }
    }
    pub fn check(&mut self) -> Result<(), SavedSelectError> {
        if (self.cancelled)() {
            return Err(SavedSelectError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }
    pub fn syntax(&mut self, bytes: usize) -> Result<(), SavedSelectError> {
        self.check()?;
        self.syntax_bytes = self
            .syntax_bytes
            .checked_add(bytes)
            .filter(|bytes| *bytes <= self.limits.max_bytes)
            .ok_or(SavedSelectError::SyntaxLimit)?;
        Ok(())
    }
    pub fn text(&mut self, parts: &[&str]) -> Result<String, SavedSelectError> {
        let length = parts
            .iter()
            .try_fold(0_usize, |n, part| n.checked_add(part.len()))
            .filter(|n| *n <= self.limits.max_bytes)
            .ok_or(SavedSelectError::SyntaxLimit)?;
        self.syntax(length)?;
        self.budget.charge(Resource::Bytes, length as u64)?;
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| SavedSelectError::Allocation)?;
        for part in parts {
            text.push_str(part);
        }
        Ok(text)
    }
    fn parsing(&mut self, bytes: usize) -> Result<(), SavedSelectError> {
        self.check()?;
        self.budget.charge(
            Resource::Steps,
            bytes as u64 + self.limits.max_tokens as u64,
        )?;
        // An AST/name allowance, not an allocator quota for the upstream parser.
        // Its temporary allocations have independent byte/token/depth ceilings.
        self.budget.charge_allocation(
            self.limits.max_tokens as u64,
            size_of::<Expression>() as u64,
        )?;
        self.budget
            .charge_allocation(bytes as u64, PARSED_TEXT_COPIES)?;
        Ok(())
    }
    pub fn expression(&mut self, value: &str) -> Result<Expression, SavedSelectError> {
        // Parentheses reject clause/list/statement injection; the newline ends
        // a trailing line comment before our closing delimiter.
        let text = self.text(&["(", value, "\n)"])?;
        self.parsing(value.len())?;
        let original = parse_expression(value, self.limits)?;
        self.parsing(text.len())?;
        let parsed = parse_expression(&text, self.limits)?;
        self.check()?;
        // Guard delimiters must not repair unmatched stored parentheses or
        // otherwise change the expression's owned meaning.
        if original != parsed {
            return Err(SavedSelectError::Unsupported);
        }
        Ok(original)
    }
    pub fn name(&mut self, value: &str) -> Result<Name, SavedSelectError> {
        check_name(value)?;
        let mut query = self.select(&["SELECT 0 AS [", value, "]"])?;
        match query.projection.pop() {
            Some(Projection::Expression {
                alias: Some(name), ..
            }) => Ok(name),
            _ => Err(SavedSelectError::InvalidMetadata),
        }
    }
    pub fn select(&mut self, parts: &[&str]) -> Result<Select, SavedSelectError> {
        let text = self.text(parts)?;
        self.parsing(text.len())?;
        let query = parse_select(&text, self.limits)?.into_query();
        self.check()?;
        Ok(query)
    }
    pub fn query(&mut self, parts: &[&str]) -> Result<Query, SavedSelectError> {
        let text = self.text(parts)?;
        self.parsing(text.len())?;
        let query = parse_query(&text, self.limits)?.into_query();
        self.check()?;
        Ok(query)
    }
    pub fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, SavedSelectError> {
        self.check()?;
        self.budget
            .charge_allocation(count as u64, size_of::<T>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| SavedSelectError::Allocation)?;
        Ok(values)
    }
}

pub(super) fn check_name(value: &str) -> Result<(), SavedSelectError> {
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']' | '.'))
    {
        return Err(SavedSelectError::Unsupported);
    }
    Ok(())
}
