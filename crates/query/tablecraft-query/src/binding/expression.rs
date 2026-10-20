use super::{Binder, resolve};
use crate::profile::{MAX_IN_VALUES, MAX_TEXT_BYTES};
use crate::{
    QueryError,
    plan::{Expr, Slot},
};
use tablecraft_sql::select::Expression as E;

impl Binder<'_> {
    pub(super) fn expression(
        &mut self,
        expr: &E,
        scopes: &[&[Slot]],
        aggregates: bool,
        depth: usize,
    ) -> Result<Expr, QueryError> {
        self.tick(depth)?;
        Ok(match expr {
            E::Field(field) => {
                for (depth, scope) in scopes.iter().enumerate() {
                    let ordinary = resolve(field, scope)?;
                    let member = if field.name.as_str().eq_ignore_ascii_case("Value") {
                        if let Some(name) = &field.qualifier {
                            super::multivalue::resolve_member(None, name.as_str(), scope)?
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if ordinary.is_some() && member.is_some() {
                        return Err(QueryError::AmbiguousName);
                    }
                    if let Some(index) = ordinary.or(member) {
                        return Ok(Expr::Field { depth, index });
                    }
                }
                if field.qualifier.is_some() {
                    return Err(QueryError::UnknownName);
                }
                Expr::Parameter(
                    self.parameter_scope
                        .iter()
                        .copied()
                        .find(|index| {
                            let p = &self.parameters[*index];
                            crate::profile::SOURCE_SCHEMA_POLICY
                                .names
                                .equals(p.name.as_str(), field.name.as_str())
                        })
                        .ok_or(QueryError::UnknownName)?,
                )
            }
            E::Member {
                expression,
                member,
                bang: false,
            } if member.as_str().eq_ignore_ascii_case("Value") => {
                let E::Field(field) = expression.as_ref() else {
                    return Err(QueryError::Unsupported);
                };
                for (depth, scope) in scopes.iter().enumerate() {
                    if let Some(index) = super::multivalue::resolve_member(
                        field.qualifier.as_ref().map(|q| q.as_str()),
                        field.name.as_str(),
                        scope,
                    )? {
                        return Ok(Expr::Field { depth, index });
                    }
                }
                return Err(QueryError::UnknownName);
            }
            E::Null
            | E::Boolean(_)
            | E::Integer(_)
            | E::Text(_)
            | E::Number(_)
            | E::DateLiteral(_)
            | E::GuidLiteral(_) => {
                if matches!(expr, E::Text(t) | E::Number(t) | E::DateLiteral(t) | E::GuidLiteral(t) if t.len() > MAX_TEXT_BYTES)
                {
                    return Err(QueryError::StructureLimit);
                }
                if let E::Text(t) | E::Number(t) | E::DateLiteral(t) | E::GuidLiteral(t) = expr {
                    self.charge(t.len())?;
                }
                // Reject unmaterializable literal spellings even on empty sources.
                if matches!(expr, E::Number(_) | E::DateLiteral(_) | E::GuidLiteral(_)) {
                    use tablecraft_core::limits::ResourceBudget;
                    use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
                    let mut budget = ResourceBudget::new(0, 0, 1);
                    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .evaluate(expr)?;
                }
                Expr::Literal(expr.clone())
            }
            E::Unary {
                operator,
                expression,
            } => Expr::Unary(
                *operator,
                Box::new(self.expression(expression, scopes, aggregates, depth + 1)?),
            ),
            E::Binary {
                left,
                operator,
                right,
            } => Expr::Binary(
                *operator,
                Box::new(self.expression(left, scopes, aggregates, depth + 1)?),
                Box::new(self.expression(right, scopes, aggregates, depth + 1)?),
            ),
            E::Like {
                expression,
                pattern,
                negated,
            } => Expr::Like(
                Box::new(self.expression(expression, scopes, aggregates, depth + 1)?),
                Box::new(self.expression(pattern, scopes, aggregates, depth + 1)?),
                *negated,
            ),
            E::IsNull {
                expression,
                negated,
            } => Expr::IsNull(
                Box::new(self.expression(expression, scopes, aggregates, depth + 1)?),
                *negated,
            ),
            E::Between {
                expression,
                first_bound,
                second_bound,
                negated,
            } => Expr::Between(
                Box::new(self.expression(expression, scopes, aggregates, depth + 1)?),
                Box::new(self.expression(first_bound, scopes, aggregates, depth + 1)?),
                Box::new(self.expression(second_bound, scopes, aggregates, depth + 1)?),
                *negated,
            ),
            E::InList {
                expression,
                values,
                negated,
            } => {
                if values.is_empty() || values.len() > MAX_IN_VALUES {
                    return Err(QueryError::StructureLimit);
                }
                Expr::InList(
                    Box::new(self.expression(expression, scopes, aggregates, depth + 1)?),
                    values
                        .iter()
                        .map(|e| self.expression(e, scopes, aggregates, depth + 1))
                        .collect::<Result<_, _>>()?,
                    *negated,
                )
            }
            E::Call {
                qualifier,
                name,
                arguments,
            } => self.call(qualifier, name, arguments, scopes, aggregates, depth)?,
            E::Subquery(q) => {
                let plan = self.read(q, scopes, depth + 1)?;
                if plan.names.len() != 1 {
                    return Err(QueryError::Width);
                }
                Expr::Subquery(Box::new(plan))
            }
            E::Exists(q) => Expr::Exists(Box::new(self.read(q, scopes, depth + 1)?)),
            E::InQuery {
                expression,
                query,
                negated,
            } => {
                let plan = self.read(query, scopes, depth + 1)?;
                if plan.names.len() != 1 {
                    return Err(QueryError::Width);
                }
                Expr::InQuery(
                    Box::new(self.expression(expression, scopes, aggregates, depth + 1)?),
                    Box::new(plan),
                    *negated,
                )
            }
            E::Quantified {
                expression,
                operator,
                quantifier,
                query,
            } => {
                let plan = self.read(query, scopes, depth + 1)?;
                if plan.names.len() != 1 {
                    return Err(QueryError::Width);
                }
                Expr::Quantified(
                    Box::new(self.expression(expression, scopes, aggregates, depth + 1)?),
                    *operator,
                    *quantifier,
                    Box::new(plan),
                )
            }
            _ => return Err(QueryError::Unsupported),
        })
    }
}
