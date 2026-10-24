//! Named collection NULL criteria use container membership, not nullable member values.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, MemberSlot, Slot},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_sql::select::{BinaryOperator as B, Expression, FieldReference, UnaryOperator as U};

impl Binder<'_> {
    pub(in crate::binding) fn filter_expression(
        &mut self,
        expression: &Expression,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<Expr, QueryError> {
        match expression {
            Expression::Unary {
                operator: U::Not,
                expression,
            } => {
                self.tick(depth)?;
                Ok(Expr::Unary(
                    U::Not,
                    Box::new(self.filter_expression(expression, scopes, depth + 1)?),
                ))
            }
            Expression::Binary {
                operator: operator @ (B::And | B::Or | B::Xor | B::Equivalence | B::Implication),
                left,
                right,
            } => {
                self.tick(depth)?;
                Ok(Expr::Binary(
                    *operator,
                    Box::new(self.filter_expression(left, scopes, depth + 1)?),
                    Box::new(self.filter_expression(right, scopes, depth + 1)?),
                ))
            }
            Expression::IsNull {
                expression: value,
                negated,
            } => {
                if let Expression::Field(field) = value.as_ref()
                    && let Some(index) =
                        named_collection(field, scopes.first().copied().unwrap_or(&[]))?
                {
                    self.tick(depth)?;
                    let value = self.field(0, index, scopes[0])?;
                    return Ok(Expr::CollectionEmpty(Box::new(value), *negated));
                }
                self.expression(expression, scopes, false, depth)
            }
            _ => self.expression(expression, scopes, false, depth),
        }
    }

    pub(super) fn collect_presence(
        &mut self,
        expression: &Expr,
        used: &mut [bool],
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        Ok(match expression {
            Expr::CollectionEmpty(value, _) => {
                let Expr::Field {
                    depth: 0, index, ..
                } = value.as_ref()
                else {
                    return Err(QueryError::Unsupported);
                };
                *used.get_mut(*index).ok_or(QueryError::Source)? = true;
                true
            }
            Expr::Unary(U::Not, value) => self.collect_presence(value, used, depth + 1)?,
            Expr::Binary(
                B::And | B::Or | B::Xor | B::Equivalence | B::Implication,
                left,
                right,
            ) => {
                let left = self.collect_presence(left, used, depth + 1)?;
                let right = self.collect_presence(right, used, depth + 1)?;
                left || right
            }
            _ => false,
        })
    }
}

fn named_collection(field: &FieldReference, slots: &[Slot]) -> Result<Option<usize>, QueryError> {
    let matches_field = |slot: &Slot| {
        SOURCE_SCHEMA_POLICY
            .names
            .equals(&slot.name, field.name.as_str())
            && field.qualifier.as_ref().is_none_or(|qualifier| {
                slot.qualifier
                    .as_deref()
                    .is_some_and(|name| SOURCE_SCHEMA_POLICY.names.equals(name, qualifier.as_str()))
            })
    };
    let mut parents = slots
        .iter()
        .filter(|slot| slot.member.is_none() && matches_field(slot));
    let Some(parent) = parents.next() else {
        return Ok(None);
    };
    if parents.next().is_some() {
        return Err(QueryError::AmbiguousName);
    }
    if parent.available {
        return Ok(None);
    }
    if let Some(qualifier) = &field.qualifier
        && super::resolve_member(None, qualifier.as_str(), field.name.as_str(), slots)?.is_some()
    {
        return Err(QueryError::AmbiguousName);
    }

    Ok(slots
        .iter()
        .filter(|slot| slot.available)
        .enumerate()
        .find_map(|(index, slot)| {
            (matches!(slot.member, Some(MemberSlot::Named(_))) && matches_field(slot))
                .then_some(index)
        }))
}
