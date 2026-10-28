use super::{SavedSelectError, identifier};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    ParseError, ParseErrorKind, ParseLimits, SourceLocation, parse_expression, parse_query,
    parse_select,
    query::Query,
    select::{Expression, Name, Projection, Select, SelectPredicate, TableSource},
};

const PARSED_TEXT_COPIES: u64 = 2;
const PROJECTION_GUARD: &str = "__tablecraft_projection_guard";

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
    pub fn projection(&mut self, value: &str) -> Result<Projection, SavedSelectError> {
        // Explicit ALL fixes the modifier slot. The final FROM/newline prevents
        // stored clauses, terminators or comments from replacing the guard.
        // No closing delimiter can repair malformed stored parentheses.
        let mut query = self.select(&["SELECT ALL ", value, "\nFROM [", PROJECTION_GUARD, "]"])?;
        let guarded = matches!(
            query.from.as_slice(),
            [TableSource::Table(table)]
                if table.name.as_str() == PROJECTION_GUARD
                    && table.alias.is_none()
                    && table.database.is_none()
        );
        if !guarded
            || query.predicate != SelectPredicate::All
            || query.projection.len() != 1
            || !query.group_by.is_empty()
            || query.having.is_some()
            || query.into.is_some()
            || query.into_external.is_some()
            || query.external.is_some()
            || query.owner_access
            || query.selection.is_some()
            || !query.order_by.is_empty()
        {
            return Err(SavedSelectError::InvalidMetadata);
        }

        match query.projection.pop() {
            Some(projection @ Projection::Wildcard(_))
            | Some(projection @ Projection::Field { alias: None, .. })
            | Some(projection @ Projection::Expression { alias: None, .. }) => Ok(projection),
            _ => Err(SavedSelectError::InvalidMetadata),
        }
    }

    // Retain the former parser carrier's conservative syntax/work allowance,
    // without allocating or parsing synthetic SQL. Stored clauses still parse.
    pub fn construction(&mut self, bytes: usize, tokens: usize) -> Result<(), SavedSelectError> {
        if bytes > self.limits.max_bytes {
            return Err(SavedSelectError::SyntaxLimit);
        }
        self.syntax(bytes)?;
        self.budget.charge(Resource::Bytes, bytes as u64)?;
        self.parsing(bytes)?;

        let message = if tokens > self.limits.max_tokens {
            Some("SQL token limit exceeded")
        } else if self.limits.max_depth == 0 {
            Some("SQL expression depth limit exceeded")
        } else {
            None
        };

        if let Some(message) = message {
            return Err(ParseError {
                kind: ParseErrorKind::Limit,
                message,
                location: SourceLocation { line: 1, column: 1 },
            }
            .into());
        }

        Ok(())
    }

    pub fn name(&mut self, value: &str) -> Result<Name, SavedSelectError> {
        let value = identifier::literal(value)?;
        let bytes = "SELECT 0 AS []"
            .len()
            .checked_add(value.len())
            .ok_or(SavedSelectError::SyntaxLimit)?;
        self.construction(bytes, 4)?;
        let name = Name::from_identifier(value, self.limits)?;
        self.check()?;
        Ok(name)
    }

    pub fn parameter_name(&mut self, value: &str) -> Result<Name, SavedSelectError> {
        let bytes = "PARAMETERS [] LONG; SELECT 0"
            .len()
            .checked_add(value.len())
            .ok_or(SavedSelectError::SyntaxLimit)?;
        self.construction(bytes, 6)?;
        let name = Name::from_contents(value, self.limits)?;
        self.check()?;
        Ok(name)
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

    pub fn source_query<'s>(
        &mut self,
        source: &'s str,
    ) -> Result<(Query, &'s str), SavedSelectError> {
        let text = self.text(&[source])?;
        self.parsing(text.len())?;
        let parsed = parse_query(&text, self.limits)?;
        let statement_end = parsed.statement_source().len();
        self.check()?;
        Ok((parsed.into_query(), &source[..statement_end]))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_names_keep_literal_and_prompt_policies() {
        let mut budget = ResourceBudget::new(1 << 24, 100, 1 << 20);
        let mut context = Context::new(&mut budget, &|| false, ParseLimits::default());
        assert_eq!(
            context.name("[Order Details]").unwrap().as_str(),
            "Order Details"
        );
        assert_eq!(
            context.parameter_name("Enter a value!").unwrap().as_str(),
            "Enter a value!"
        );

        for value in ["a.b", "Forms!Value", "a]b", ""] {
            assert_eq!(context.name(value), Err(SavedSelectError::Unsupported));
        }
    }

    #[test]
    fn owned_name_construction_retains_cancellation_and_parser_ceilings() {
        let mut budget = ResourceBudget::new(1 << 24, 100, 1 << 20);
        let mut context = Context::new(&mut budget, &|| true, ParseLimits::default());
        assert_eq!(context.name("Valid"), Err(SavedSelectError::Cancelled));

        for limits in [
            ParseLimits {
                max_tokens: 3,
                ..ParseLimits::default()
            },
            ParseLimits {
                max_depth: 0,
                ..ParseLimits::default()
            },
        ] {
            let mut context = Context::new(&mut budget, &|| false, limits);
            assert!(
                matches!(context.name("Valid"), Err(SavedSelectError::Parse(error))
                if error.kind == ParseErrorKind::Limit)
            );
        }

        let mut budget = ResourceBudget::new(0, 100, 1 << 20);
        let mut context = Context::new(&mut budget, &|| false, ParseLimits::default());
        assert!(matches!(
            context.name("Valid"),
            Err(SavedSelectError::Limit(_))
        ));
    }
}
