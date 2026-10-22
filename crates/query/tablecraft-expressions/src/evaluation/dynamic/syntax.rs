//! Budgeted syntax staging and closed-name/effect validation for Eval strings.
use super::Evaluator;
use crate::{
    EvalError,
    arrays::sql_arguments,
    functions::{Builtin, math::MathFunction},
};
use tablecraft_core::limits::Resource;
use tablecraft_sql::{
    ParseError, ParseErrorKind, ParseLimits, parse_expression_strict,
    select::{Expression, FunctionArgument},
};

const AST_TEXT_COPIES: u64 = 2;

impl Evaluator<'_> {
    fn dynamic_parse_limits(
        &mut self,
        bytes: usize,
        depth: usize,
    ) -> Result<ParseLimits, EvalError> {
        self.check(depth)?;
        if bytes > ParseLimits::MAX_BYTES {
            return Err(EvalError::DynamicLimit);
        }
        let remaining = self.limits.max_depth.saturating_sub(depth);
        if remaining == 0 {
            return Err(EvalError::DepthLimit);
        }
        let syntax_units = bytes.saturating_add(1);
        let tokens = syntax_units.min(ParseLimits::MAX_TOKENS);
        self.budget
            .charge(Resource::Steps, (bytes + tokens) as u64)?;
        self.budget
            .charge_allocation(bytes as u64, AST_TEXT_COPIES)?;
        self.budget
            .charge_allocation(syntax_units as u64, size_of::<Expression>() as u64)?;
        Ok(ParseLimits {
            max_bytes: bytes,
            max_tokens: tokens,
            max_depth: remaining,
        })
    }

    pub(super) fn parse_dynamic(
        &mut self,
        text: &str,
        depth: usize,
    ) -> Result<Expression, EvalError> {
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        let limits = self.dynamic_parse_limits(text.len(), depth)?;
        let original = parse_expression_strict(text, limits).map_err(adapt)?;
        self.check(depth)?;
        Ok(original)
    }

    pub(super) fn validate_dynamic(
        &mut self,
        expression: &Expression,
        depth: usize,
    ) -> Result<(), EvalError> {
        self.check(depth)?;
        match expression {
            Expression::Field(_) => return Err(EvalError::UnboundField),
            Expression::Subquery(_)
            | Expression::Exists(_)
            | Expression::InQuery { .. }
            | Expression::Quantified { .. }
            | Expression::Member { .. }
            | Expression::Identity => {
                return Err(EvalError::UnsupportedExpression);
            }
            Expression::Call {
                qualifier,
                name,
                arguments,
            } => {
                if !sql_arguments::is_constructor(qualifier, name) {
                    let builtin = qualifier
                        .is_empty()
                        .then(|| Builtin::resolve(name.as_str()))
                        .flatten()
                        .ok_or(EvalError::UnsupportedExpression)?;
                    if self.repeatable_dynamic
                        && matches!(
                            builtin,
                            Builtin::Clock(_) | Builtin::Math(MathFunction::Rnd)
                        )
                    {
                        return Err(EvalError::DynamicNotRepeatable);
                    }
                }
                for argument in arguments {
                    if let FunctionArgument::Expression(value) = argument {
                        self.validate_dynamic(value, depth + 1)?;
                    }
                }
            }
            Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
                self.validate_dynamic(expression, depth + 1)?;
            }
            Expression::Binary { left, right, .. } => {
                self.validate_dynamic(left, depth + 1)?;
                self.validate_dynamic(right, depth + 1)?;
            }
            Expression::Like {
                expression,
                pattern,
                ..
            } => {
                self.validate_dynamic(expression, depth + 1)?;
                self.validate_dynamic(pattern, depth + 1)?;
            }
            Expression::Between {
                expression,
                first_bound,
                second_bound,
                ..
            } => {
                for value in [expression, first_bound, second_bound] {
                    self.validate_dynamic(value, depth + 1)?;
                }
            }
            Expression::InList {
                expression, values, ..
            } => {
                self.validate_dynamic(expression, depth + 1)?;
                for value in values {
                    self.validate_dynamic(value, depth + 1)?;
                }
            }
            Expression::Null
            | Expression::Boolean(_)
            | Expression::Integer(_)
            | Expression::Number(_)
            | Expression::Text(_)
            | Expression::DateLiteral(_)
            | Expression::GuidLiteral(_) => {}
        }
        Ok(())
    }
}

fn adapt(error: ParseError) -> EvalError {
    if error.kind == ParseErrorKind::Limit {
        EvalError::DynamicLimit
    } else {
        EvalError::DynamicSyntax
    }
}
