mod derived;
mod table;

use super::Binder;
use crate::profile::SOURCE_SCHEMA_POLICY;
use crate::{
    QueryError,
    external_sources::SourceNamespace,
    plan::{Relation, RelationNode, Slot},
};
use tablecraft_sql::select::{Expression, JoinKind, TableSource};

impl Binder<'_> {
    pub(super) fn relation(
        &mut self,
        source: &TableSource,
        outer: &[&[Slot]],
        depth: usize,
        namespace: SourceNamespace,
    ) -> Result<Relation, QueryError> {
        self.tick(depth)?;
        if let TableSource::Table(table) = source {
            return self.table_relation(table, depth, namespace);
        }
        if matches!(source, TableSource::Join { .. }) {
            return self.join_relation(source, outer, depth, namespace);
        }
        self.non_table_relation(source, outer, depth, namespace)
    }

    fn non_table_relation(
        &mut self,
        source: &TableSource,
        outer: &[&[Slot]],
        depth: usize,
        namespace: SourceNamespace,
    ) -> Result<Relation, QueryError> {
        let relation = match source {
            TableSource::Table(_) => unreachable!("table sources use their dedicated entry"),
            TableSource::Derived { query, alias } => {
                if alias
                    .as_ref()
                    .is_some_and(|a| a.as_str().len() > SOURCE_SCHEMA_POLICY.max_name_bytes)
                {
                    return Err(QueryError::StructureLimit);
                }
                // Derived sources are not lateral; outer references are deliberately rejected.
                let read = self.read(query, &[], depth + 1)?;
                return self.derived_relation(read, alias.as_ref().map(|name| name.as_str()));
            }
            TableSource::Group { source, alias } => {
                if alias
                    .as_ref()
                    .is_some_and(|a| a.as_str().len() > SOURCE_SCHEMA_POLICY.max_name_bytes)
                {
                    return Err(QueryError::StructureLimit);
                }
                let mut rel = self.relation(source, outer, depth + 1, namespace)?;
                if let Some(alias) = alias {
                    for slot in &mut rel.slots {
                        slot.qualifier = Some(alias.as_str().into());
                    }
                }
                rel
            }
            TableSource::Join { .. } => unreachable!("joins use their dedicated entry"),
        };
        self.finish_relation(relation)
    }

    // Keep condition binding and owned slot construction off the recursive
    // source traversal stack, just as the ordinary named-table entry does.
    #[inline(never)]
    fn join_relation(
        &mut self,
        source: &TableSource,
        outer: &[&[Slot]],
        depth: usize,
        namespace: SourceNamespace,
    ) -> Result<Relation, QueryError> {
        let TableSource::Join {
            left,
            right,
            kind,
            condition,
        } = source
        else {
            unreachable!("the join entry requires a join source");
        };
        let left = self.relation(left, outer, depth + 1, namespace)?;
        let right = self.relation(right, outer, depth + 1, namespace)?;
        self.finish_join_relation(left, right, *kind, condition, outer, depth)
    }

    fn finish_join_relation(
        &mut self,
        left: Relation,
        right: Relation,
        kind: JoinKind,
        condition: &Expression,
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<Relation, QueryError> {
        let mut slots = combine_slots(&left.slots, &right.slots)?;
        let nullable = match kind {
            JoinKind::Left => left.slots.len()..slots.len(),
            JoinKind::Right => 0..left.slots.len(),
            JoinKind::Inner => 0..0,
        };
        for slot in &mut slots[nullable] {
            slot.output_type = slot.output_type.with_null();
        }
        let scopes = std::iter::once(slots.as_slice())
            .chain(outer.iter().copied())
            .collect::<Vec<_>>();
        let condition = self.expression(condition, &scopes, false, depth + 1)?;
        self.finish_relation(Relation {
            slots,
            node: RelationNode::Join {
                left: Box::new(left),
                right: Box::new(right),
                kind,
                condition,
            },
        })
    }

    fn finish_relation(&mut self, relation: Relation) -> Result<Relation, QueryError> {
        if !valid_slot_width(relation.slots.iter()) {
            return Err(QueryError::StructureLimit);
        }
        self.charge(
            relation
                .slots
                .iter()
                .map(|s| {
                    s.name.len()
                        + s.qualifier.as_ref().map_or(0, String::len)
                        + std::mem::size_of::<Slot>()
                })
                .sum(),
        )?;
        Ok(relation)
    }
}
pub(super) fn combine_slots(left: &[Slot], right: &[Slot]) -> Result<Vec<Slot>, QueryError> {
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
    left.iter()
        .chain(right)
        .enumerate()
        .map(|(position, slot)| {
            let collection_index = match slot.collection_index {
                Some(index) if position >= left.len() => Some(
                    index
                        .checked_add(collection_offset)
                        .ok_or(QueryError::StructureLimit)?,
                ),
                index => index,
            };
            Ok(Slot {
                output_type: slot.output_type,
                declaration: slot.declaration,
                text_policy: slot.text_policy,
                available: slot.available,
                collection_index,
                collection_origin: slot.collection_origin,
                retained_member: slot.retained_member,
                member: slot.member.clone(),
                name: slot.name.clone(),
                qualifier: slot.qualifier.clone(),
            })
        })
        .collect()
}

// Every logical output may add at most one retained virtual Value slot. The
// public source/output ceiling remains unchanged; only the private namespace
// receives this separate, checked allowance.
fn valid_slot_width<'a>(slots: impl Iterator<Item = &'a Slot>) -> bool {
    let mut logical = 0usize;
    let mut retained = 0usize;
    for slot in slots {
        if slot.retained_member {
            retained += 1;
        } else {
            logical += 1;
        }
    }
    logical <= SOURCE_SCHEMA_POLICY.max_columns && retained <= SOURCE_SCHEMA_POLICY.max_columns
}
