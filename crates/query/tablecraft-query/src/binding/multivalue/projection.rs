//! Bare collection names are projection leaves, never scalar expressions.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_sql::select::{Expression, FieldReference};

impl Binder<'_> {
    pub(in crate::binding) fn collection_projection(
        &mut self,
        expression: &Expression,
        slots: &[Slot],
        depth: usize,
    ) -> Result<Option<Expr>, QueryError> {
        let Expression::Field(field) = expression else {
            return Ok(None);
        };
        self.collection_field(field, slots, depth)
    }

    pub(in crate::binding) fn collection_field(
        &mut self,
        field: &FieldReference,
        slots: &[Slot],
        depth: usize,
    ) -> Result<Option<Expr>, QueryError> {
        let mut found = None;
        for slot in slots {
            self.tick(depth)?;
            let output_type = slot.output_type.conservative();
            if slot.member.is_none()
                && SOURCE_SCHEMA_POLICY
                    .names
                    .equals(&slot.name, field.name.as_str())
                && field.qualifier.as_ref().is_none_or(|qualifier| {
                    slot.qualifier.as_ref().is_some_and(|name| {
                        SOURCE_SCHEMA_POLICY.names.equals(name, qualifier.as_str())
                    })
                })
            {
                if found.is_some() {
                    return Err(QueryError::AmbiguousName);
                }
                found = Some(if output_type.is_multivalue() {
                    Some(Expr::Multivalue {
                        index: slot.collection_index.ok_or(QueryError::Source)?,
                        output_type,
                    })
                } else {
                    None
                });
            }
        }
        Ok(found.flatten())
    }
}
