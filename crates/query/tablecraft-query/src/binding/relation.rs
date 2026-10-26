mod table;

use super::Binder;
use crate::profile::SOURCE_SCHEMA_POLICY;
use crate::{
    QueryError,
    external_sources::SourceNamespace,
    plan::{ProfiledType, Relation, RelationNode, Slot},
};
use tablecraft_sql::select::{JoinKind, TableSource};

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
                Relation {
                    slots: read
                        .names
                        .iter()
                        .zip(&read.text_policies)
                        .zip(&read.output_types)
                        .zip(&read.declarations)
                        .enumerate()
                        .map(|(index, (((n, policy), _), declaration))| Slot {
                            output_type: ProfiledType {
                                materialized: read.materialized_types[index],
                                declared: read.declared_types[index],
                            },
                            declaration: *declaration,
                            text_policy: *policy,
                            available: !read.output_types[index].is_multivalue(),
                            collection_index: read.output_types[index].is_multivalue().then(|| {
                                read.output_types[..index]
                                    .iter()
                                    .filter(|output| output.is_multivalue())
                                    .count()
                            }),
                            member: None,
                            name: n.clone(),
                            qualifier: alias.as_ref().map(|a| a.as_str().into()),
                        })
                        .collect(),
                    node: RelationNode::Derived(Box::new(read)),
                }
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
            TableSource::Join {
                left,
                right,
                kind,
                condition,
            } => {
                let left = self.relation(left, outer, depth + 1, namespace)?;
                let right = self.relation(right, outer, depth + 1, namespace)?;
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
                Relation {
                    slots,
                    node: RelationNode::Join {
                        left: Box::new(left),
                        right: Box::new(right),
                        kind: *kind,
                        condition,
                    },
                }
            }
        };
        self.finish_relation(relation)
    }

    fn finish_relation(&mut self, relation: Relation) -> Result<Relation, QueryError> {
        if relation.slots.len() > SOURCE_SCHEMA_POLICY.max_columns {
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
    if left.len() + right.len() > SOURCE_SCHEMA_POLICY.max_columns {
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
    Ok(left
        .iter()
        .chain(right)
        .map(|s| Slot {
            output_type: s.output_type,
            declaration: s.declaration,
            text_policy: s.text_policy,
            available: s.available,
            collection_index: s.collection_index,
            member: s.member.clone(),
            name: s.name.clone(),
            qualifier: s.qualifier.clone(),
        })
        .collect())
}
