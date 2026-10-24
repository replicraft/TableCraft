//! Named collection NULL criteria use container membership, not nullable member values.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, MemberSlot, Slot},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_sql::select::{Expression, FieldReference};

mod children;

impl Binder<'_> {
    pub(in crate::binding) fn filter_expression(
        &mut self,
        expression: &Expression,
        scopes: &[&[Slot]],
        depth: usize,
    ) -> Result<Expr, QueryError> {
        self.with_collection_criteria(true, |binder| {
            binder.expression(expression, scopes, false, depth)
        })
    }

    pub(in crate::binding) fn with_collection_criteria<T>(
        &mut self,
        enabled: bool,
        build: impl FnOnce(&mut Self) -> Result<T, QueryError>,
    ) -> Result<T, QueryError> {
        let previous = self.collection_criteria;
        self.collection_criteria = enabled;
        let result = build(self);
        self.collection_criteria = previous;
        result
    }

    pub(in crate::binding) fn collection_null_test(
        &mut self,
        expression: &Expression,
        scopes: &[&[Slot]],
        negated: bool,
        depth: usize,
    ) -> Result<Option<Expr>, QueryError> {
        if self.collection_criteria
            && let Expression::Field(field) = expression
            && let Some(index) = named_collection(field, scopes.first().copied().unwrap_or(&[]))?
        {
            self.tick(depth)?;
            let field = self.field(0, index, scopes[0])?;
            return Ok(Some(Expr::CollectionEmpty(Box::new(field), negated)));
        }
        Ok(None)
    }

    pub(super) fn collect_presence(
        &mut self,
        expression: &Expr,
        used: &mut [bool],
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick(depth)?;
        if let Expr::CollectionEmpty(value, _) = expression {
            let Expr::Field {
                depth: 0, index, ..
            } = value.as_ref()
            else {
                return Err(QueryError::Unsupported);
            };
            *used.get_mut(*index).ok_or(QueryError::Source)? = true;
            return Ok(true);
        }
        let mut found = false;
        for child in children::children(expression) {
            found |= self.collect_presence(child, used, depth + 1)?;
        }
        Ok(found)
    }

    pub(super) fn resolve_presence(
        &mut self,
        expression: &mut Expr,
        fields: &[(usize, usize)],
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        if let Expr::CollectionEmpty(value, negated) = expression {
            let Expr::Field {
                depth: 0, index, ..
            } = value.as_ref()
            else {
                return Err(QueryError::Unsupported);
            };
            let collection = fields
                .iter()
                .find(|(field, _)| field == index)
                .map(|(_, collection)| *collection)
                .ok_or(QueryError::Source)?;
            *expression = Expr::ParentPresence {
                collection,
                negated: *negated,
            };
            return Ok(());
        }
        for child in children::children_mut(expression) {
            self.resolve_presence(child, fields, depth + 1)?;
        }
        Ok(())
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
