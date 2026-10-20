use super::{Binder, resolve};
use crate::profile::{MAX_IN_VALUES, MAX_TEXT_BYTES};
use crate::{
    QueryError,
    plan::{Aggregate, Expr, Slot},
};
use tablecraft_sql::select::{Expression as E, FunctionArgument as A};

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
                    if let Some(index) = resolve(field, scope)? {
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
            } => {
                if self.retrieval && qualifier.is_empty() {
                    use tablecraft_expressions::functions::Builtin;
                    match (Builtin::resolve(name.as_str()), arguments.as_slice()) {
                        (Some(Builtin::IsNull), [A::Expression(value)]) => {
                            return Ok(Expr::IsNull(
                                Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                                false,
                            ));
                        }
                        (
                            Some(Builtin::IIf),
                            [
                                A::Expression(condition),
                                A::Expression(truepart),
                                A::Expression(falsepart),
                            ],
                        ) => {
                            return Ok(Expr::IIf(
                                Box::new(self.expression(
                                    condition,
                                    scopes,
                                    aggregates,
                                    depth + 1,
                                )?),
                                Box::new(self.expression(
                                    truepart,
                                    scopes,
                                    aggregates,
                                    depth + 1,
                                )?),
                                Box::new(self.expression(
                                    falsepart,
                                    scopes,
                                    aggregates,
                                    depth + 1,
                                )?),
                            ));
                        }
                        (Some(Builtin::Nz), [A::Expression(value)]) => {
                            return Ok(Expr::Nz(
                                Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                                None,
                            ));
                        }
                        (Some(Builtin::Nz), [A::Expression(value), A::Expression(replacement)]) => {
                            return Ok(Expr::Nz(
                                Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                                Some(Box::new(self.expression(
                                    replacement,
                                    scopes,
                                    aggregates,
                                    depth + 1,
                                )?)),
                            ));
                        }
                        (Some(Builtin::Text(function)), arguments)
                            if function.accepts_arity(arguments.len())
                                && arguments.iter().all(|arg| matches!(arg, A::Expression(_))) =>
                        {
                            self.charge(arguments.len() * std::mem::size_of::<Expr>())?;
                            let mut values = Vec::new();
                            values
                                .try_reserve_exact(arguments.len())
                                .map_err(|_| QueryError::Allocation)?;
                            for argument in arguments {
                                let A::Expression(expr) = argument else {
                                    return Err(QueryError::Unsupported);
                                };
                                values.push(self.expression(
                                    expr,
                                    scopes,
                                    aggregates,
                                    depth + 1,
                                )?);
                            }
                            return Ok(Expr::TextCall(function, values));
                        }
                        (Some(_), _) => return Err(QueryError::Unsupported),
                        (None, _) => {}
                    }
                }
                if !aggregates || !qualifier.is_empty() || arguments.len() != 1 {
                    return Err(QueryError::Unsupported);
                }
                let aggregate = aggregate(name.as_str()).ok_or(QueryError::Unsupported)?;
                let argument = match &arguments[0] {
                    A::Wildcard if matches!(aggregate, Aggregate::Count) => None,
                    A::Expression(e) => {
                        Some(Box::new(self.expression(e, scopes, false, depth + 1)?))
                    }
                    _ => return Err(QueryError::Unsupported),
                };
                Expr::Aggregate(aggregate, argument)
            }
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

fn aggregate(name: &str) -> Option<Aggregate> {
    [
        ("COUNT", Aggregate::Count),
        ("SUM", Aggregate::Sum),
        ("AVG", Aggregate::Average),
        ("MIN", Aggregate::Minimum),
        ("MAX", Aggregate::Maximum),
    ]
    .into_iter()
    .find_map(|(spelling, kind)| name.eq_ignore_ascii_case(spelling).then_some(kind))
}
