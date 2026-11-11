//! Paid owned namespaces for JOIN and comma-separated source products.
use super::{Binder, valid_slot_width};
use crate::{
    QueryError,
    plan::{MemberSlot, Slot},
    profile::SOURCE_SCHEMA_POLICY,
};

impl Binder<'_> {
    pub(in crate::binding) fn combine_slots(
        &mut self,
        left: &[Slot],
        right: &[Slot],
    ) -> Result<Vec<Slot>, QueryError> {
        if !valid_slot_width(left.iter().chain(right)) {
            return Err(QueryError::StructureLimit);
        }
        // Repeated relation qualifiers would make qualified lookup unstable.
        if left.iter().any(|a| {
            right.iter().any(|b| {
                a.qualifier
                    .as_ref()
                    .zip(b.qualifier.as_ref())
                    .is_some_and(|(a, b)| SOURCE_SCHEMA_POLICY.names.equals(a, b))
            })
        }) {
            return Err(QueryError::AmbiguousName);
        }
        // Collection slots are independent of dense scalar rows and unavailable-name
        // order. Keep every left slot in the offset, including unprojected lists.
        let collection_offset = left
            .iter()
            .filter(|slot| slot.collection_index.is_some())
            .count();

        let recordset_offset = left.iter().filter(|slot| slot.recordset.is_some()).count();
        self.work()?;
        let width = left
            .len()
            .checked_add(right.len())
            .ok_or(QueryError::StructureLimit)?;
        self.charge(
            width
                .checked_mul(size_of::<Slot>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;

        for (position, slot) in left.iter().chain(right).enumerate() {
            self.work()?;
            let collection_index = match slot.collection_index {
                Some(index) if position >= left.len() => Some(
                    index
                        .checked_add(collection_offset)
                        .ok_or(QueryError::StructureLimit)?,
                ),
                index => index,
            };
            let member = match &slot.member {
                Some(MemberSlot::Value) => Some(MemberSlot::Value),
                Some(MemberSlot::Named(name)) => Some(MemberSlot::Named(self.copy_name(name)?)),
                None => None,
            };
            let name = self.copy_name(&slot.name)?;
            let qualifier = slot
                .qualifier
                .as_deref()
                .map(|qualifier| self.copy_name(qualifier))
                .transpose()?;

            let recordset = slot
                .recordset
                .as_ref()
                .map(|recordset| {
                    self.retain_recordset_slot(
                        recordset,
                        if position >= left.len() {
                            recordset_offset
                        } else {
                            0
                        },
                    )
                })
                .transpose()?;
            slots.push(Slot {
                recordset,
                output_type: slot.output_type,
                declaration: slot.declaration,
                text_policy: slot.text_policy,
                available: slot.available,
                collection_index,
                collection_origin: slot.collection_origin,
                retained_member: slot.retained_member,
                member,
                name,
                qualifier,
            });
        }
        Ok(slots)
    }
}

#[cfg(test)]
mod tests;
