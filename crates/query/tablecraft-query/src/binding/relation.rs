use super::Binder;
use crate::{
    QueryError,
    plan::{Relation, RelationNode, Slot},
};
use tablecraft_schema::validate_columns;
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
                if table.name.as_str().len() > 255
                    || table.alias.as_ref().is_some_and(|a| a.as_str().len() > 255)
                {
                    return Err(QueryError::StructureLimit);
                }
                let columns = self.source.columns(table.name.as_str())?;
                validate_columns(columns)?;
                self.charge(
                    columns
                        .iter()
                        .map(|c| {
                            c.name().len()
                                + 255
                                + std::mem::size_of::<Slot>()
                                + std::mem::size_of::<tablecraft_schema::Column>()
                        })
                        .sum(),
                )?;
                Relation {
                    slots: columns
                        .iter()
                        .map(|c| Slot {
                            name: c.name().into(),
                            qualifier: Some(
                                table.alias.as_ref().unwrap_or(&table.name).as_str().into(),
                            ),
                        })
                        .collect(),
                    node: RelationNode::Table {
                        name: table.name.as_str().into(),
                        columns: columns.to_vec(),
                    },
                }
            }
            TableSource::Derived { query, alias } => {
                if alias.as_ref().is_some_and(|a| a.as_str().len() > 255) {
                    return Err(QueryError::StructureLimit);
                }
                // Derived sources are not lateral; outer references are deliberately rejected.
                let read = self.read(query, &[], depth + 1)?;
                Relation {
                    slots: read
                        .names
                        .iter()
                        .map(|n| Slot {
                            name: n.clone(),
                            qualifier: alias.as_ref().map(|a| a.as_str().into()),
                        })
                        .collect(),
                    node: RelationNode::Derived(Box::new(read)),
                }
            }
            TableSource::Group { source, alias } => {
                if alias.as_ref().is_some_and(|a| a.as_str().len() > 255) {
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
        if relation.slots.len() > 256 {
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
    if left.len() + right.len() > 256 {
        return Err(QueryError::StructureLimit);
    }
    // Repeated relation qualifiers would make qualified lookup unstable.
    if left.iter().any(|a| {
        right.iter().any(|b| {
            a.qualifier
                .as_ref()
                .zip(b.qualifier.as_ref())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        })
    }) {
        return Err(QueryError::AmbiguousName);
    }
    Ok(left
        .iter()
        .chain(right)
        .map(|s| Slot {
            name: s.name.clone(),
            qualifier: s.qualifier.clone(),
        })
        .collect())
}
