use super::Binder;
use crate::profile::SOURCE_SCHEMA_POLICY;
use crate::{
    QueryError,
    plan::{Relation, RelationNode, Slot},
};
use tablecraft_schema::Column;
use tablecraft_sql::select::TableSource;

impl Binder<'_> {
    pub(super) fn relation(
        &mut self,
        source: &TableSource,
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<Relation, QueryError> {
        self.tick(depth)?;
        let relation = match source {
            TableSource::Table(table) => {
                if table.name.as_str().len() > SOURCE_SCHEMA_POLICY.max_name_bytes
                    || table
                        .alias
                        .as_ref()
                        .is_some_and(|a| a.as_str().len() > SOURCE_SCHEMA_POLICY.max_name_bytes)
                {
                    return Err(QueryError::StructureLimit);
                }
                if self.retrieval
                    && let Some(query) = self.source.query(table.name.as_str())?
                {
                    let read = self.named_read(table.name.as_str(), query, depth + 1)?;
                    let qualifier = table.alias.as_ref().unwrap_or(&table.name).as_str();
                    return self.named_relation(read, qualifier);
                }
                let columns = self.source.columns(table.name.as_str())?;
                let unavailable = self.source.unavailable_columns(table.name.as_str())?;
                super::source_schema::validate(columns, unavailable)?;
                let multivalues = self.source.multivalue_columns(table.name.as_str())?;
                let members = self.source.collection_members(table.name.as_str())?;
                super::source_schema::validate_multivalues(columns, unavailable, multivalues)?;
                super::source_schema::validate_members(columns, unavailable, multivalues, members)?;
                self.charge(
                    members
                        .iter()
                        .map(|m| {
                            2 * m.collection().as_str().len()
                                + 2 * m.column().name().len()
                                + size_of::<tablecraft_schema::CollectionMember>()
                                + size_of::<Slot>()
                                + size_of::<usize>()
                                + SOURCE_SCHEMA_POLICY.max_name_bytes
                        })
                        .sum(),
                )?;
                self.charge(
                    multivalues
                        .iter()
                        .map(|column| {
                            column.name().len()
                                + size_of::<Column>()
                                + size_of::<Slot>()
                                + size_of::<usize>()
                                + SOURCE_SCHEMA_POLICY.max_name_bytes
                        })
                        .sum(),
                )?;
                self.charge(
                    unavailable
                        .iter()
                        .map(|name| {
                            name.as_str().len()
                                + SOURCE_SCHEMA_POLICY.max_name_bytes
                                + size_of::<Slot>()
                                + size_of::<tablecraft_schema::SchemaName>()
                        })
                        .sum(),
                )?;
                self.charge(
                    columns
                        .iter()
                        .map(|c| {
                            c.name().len()
                                + SOURCE_SCHEMA_POLICY.max_name_bytes
                                + std::mem::size_of::<Slot>()
                                + std::mem::size_of::<tablecraft_schema::Column>()
                                + size_of::<usize>()
                        })
                        .sum(),
                )?;
                Relation {
                    slots: columns
                        .iter()
                        .map(|c| Slot {
                            available: true,
                            member: None,
                            name: c.name().into(),
                            qualifier: Some(
                                table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                            ),
                        })
                        .chain(unavailable.iter().map(|name| Slot {
                            available: false,
                            member: None,
                            name: name.as_str().into(),
                            qualifier: Some(
                                table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                            ),
                        }))
                        .chain(multivalues.iter().map(|column| Slot {
                            available: true,
                            member: Some("Value".into()),
                            name: column.name().into(),
                            qualifier: Some(
                                table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                            ),
                        }))
                        .chain(members.iter().map(|member| Slot {
                            available: true,
                            member: Some(member.column().name().into()),
                            name: member.collection().as_str().into(),
                            qualifier: Some(
                                table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                            ),
                        }))
                        .collect(),
                    node: RelationNode::Table {
                        name: table.name.as_str().into(),
                        columns: columns.to_vec(),
                        unavailable: unavailable.to_vec(),
                        projection: (0..columns.len()).collect(),
                        multivalues: multivalues.to_vec(),
                        members: members.to_vec(),
                        expansion: (0..multivalues.len() + members.len()).collect(),
                    },
                }
            }
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
                        .map(|n| Slot {
                            available: true,
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
                let mut rel = self.relation(source, outer, depth + 1)?;
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
                let left = self.relation(left, outer, depth + 1)?;
                let right = self.relation(right, outer, depth + 1)?;
                let slots = combine_slots(&left.slots, &right.slots)?;
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
            available: s.available,
            member: s.member.clone(),
            name: s.name.clone(),
            qualifier: s.qualifier.clone(),
        })
        .collect())
}
