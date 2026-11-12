//! One scalar relation and original target occurrence for joined mutations.
use super::Binder;
use crate::{
    QueryError,
    external_sources::SourceNamespace,
    plan::{Relation, RelationNode},
};
use std::ops::Range;
use tablecraft_schema::Column;
use tablecraft_sql::select::TableSource;

pub(crate) struct JoinedTargetSource<'a> {
    pub table: &'a str,
    pub qualifier: &'a str,
    pub columns: &'a [Column],
    pub text_collations: &'a [Option<crate::source::TextCollationId>],
}

pub(crate) struct Target {
    pub leaf: usize,
    pub columns: Range<usize>,
}

pub(super) fn bind(
    sources: &[TableSource],
    selected: &JoinedTargetSource<'_>,
    binder: &mut Binder<'_>,
) -> Result<(Relation, Target), QueryError> {
    for source in sources {
        syntax(source, 1, binder)?;
    }
    let relation = bind_sources(sources, binder, 1)?;
    let mut target = None;
    find_target(&relation, selected, &mut 0, 0, &mut target, binder)?;
    Ok((relation, target.ok_or(QueryError::UnknownName)?))
}

fn bind_sources(
    sources: &[TableSource],
    binder: &mut Binder<'_>,
    depth: usize,
) -> Result<Relation, QueryError> {
    binder.tick(depth)?;
    if let [source] = sources {
        return binder.relation(source, &[], depth, SourceNamespace::Root);
    }
    let middle = sources.len() / 2;
    if middle == 0 {
        return Err(QueryError::Unsupported);
    }
    let left = bind_sources(&sources[..middle], binder, depth + 1)?;
    let right = bind_sources(&sources[middle..], binder, depth + 1)?;
    let slots = binder.combine_slots(&left.slots, &right.slots)?;
    binder.charge(2 * size_of::<Relation>())?;
    Ok(Relation {
        slots,
        node: RelationNode::Cross(Box::new(left), Box::new(right)),
    })
}

fn syntax(source: &TableSource, depth: usize, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    binder.tick(depth)?;
    match source {
        TableSource::Table(table) if table.database.is_none() => Ok(()),
        TableSource::Join { left, right, .. } => {
            syntax(left, depth + 1, binder)?;
            syntax(right, depth + 1, binder)
        }
        TableSource::Group {
            source,
            alias: None,
        } => syntax(source, depth + 1, binder),
        _ => Err(QueryError::Unsupported),
    }
}

fn find_target(
    relation: &Relation,
    selected: &JoinedTargetSource<'_>,
    leaf: &mut usize,
    base: usize,
    target: &mut Option<Target>,
    binder: &mut Binder<'_>,
) -> Result<(), QueryError> {
    binder.work()?;
    match &relation.node {
        RelationNode::Table {
            name,
            columns,
            multivalues,
            members,
            recordsets,
            text_collations,
            ..
        } => {
            if !multivalues.is_empty()
                || !members.is_empty()
                || !recordsets.is_empty()
                || relation
                    .slots
                    .iter()
                    .any(|slot| !slot.available || slot.member.is_some())
            {
                return Err(QueryError::Unsupported);
            }
            let matches = relation
                .slots
                .first()
                .and_then(|slot| slot.qualifier.as_deref())
                .is_some_and(|name| name.eq_ignore_ascii_case(selected.qualifier));
            if matches {
                if !name.eq_ignore_ascii_case(selected.table) {
                    return Err(QueryError::Source);
                }
                if text_collations.len() != selected.text_collations.len() {
                    return Err(QueryError::SchemaChanged);
                }
                for (captured, actual) in selected.text_collations.iter().zip(text_collations) {
                    binder.work()?;
                    if captured != actual {
                        return Err(QueryError::SchemaChanged);
                    }
                }
                if !columns_match(columns, selected.columns, |amount| {
                    binder.work()?;
                    if let Some(work) = &mut binder.budget {
                        work.charge(tablecraft_core::limits::Resource::Steps, amount as u64)?;
                    }
                    Ok(())
                })? {
                    return Err(QueryError::SchemaChanged);
                }
                let end = base
                    .checked_add(relation.row_width())
                    .ok_or(QueryError::StructureLimit)?;
                if target
                    .replace(Target {
                        leaf: *leaf,
                        columns: base..end,
                    })
                    .is_some()
                {
                    return Err(QueryError::AmbiguousName);
                }
            }
            *leaf = leaf.checked_add(1).ok_or(QueryError::StructureLimit)?;
        }
        RelationNode::Join {
            left,
            right,
            condition,
            ..
        } => {
            if !crate::binding::mutation_scalar::predicate_admitted(condition) {
                return Err(QueryError::Unsupported);
            }
            find_target(left, selected, leaf, base, target, binder)?;
            let base = base
                .checked_add(left.row_width())
                .ok_or(QueryError::StructureLimit)?;
            find_target(right, selected, leaf, base, target, binder)?;
        }
        RelationNode::Cross(left, right) => {
            find_target(left, selected, leaf, base, target, binder)?;
            let base = base
                .checked_add(left.row_width())
                .ok_or(QueryError::StructureLimit)?;
            find_target(right, selected, leaf, base, target, binder)?;
        }
        _ => return Err(QueryError::Unsupported),
    }
    Ok(())
}

/// Compare the actual original target declaration, charging names and scalar facts.
pub(crate) fn columns_match(
    left: &[Column],
    right: &[Column],
    mut work: impl FnMut(usize) -> Result<(), QueryError>,
) -> Result<bool, QueryError> {
    work(1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        let amount = left
            .name()
            .len()
            .max(right.name().len())
            .checked_add(1)
            .ok_or(QueryError::StructureLimit)?;
        work(amount)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}
