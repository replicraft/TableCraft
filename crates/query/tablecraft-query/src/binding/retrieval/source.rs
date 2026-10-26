//! Validate and expand sources before allocating expression-planning scratch.
use super::Binder;
use crate::{
    QueryError,
    binding::relation::combine_slots,
    external_sources::SourceNamespace,
    plan::{Relation, RelationNode, Slot},
    profile::{MAX_FROM_ITEMS, MAX_GROUP_KEYS, PERCENT_TOTAL, SOURCE_SCHEMA_POLICY},
};
use tablecraft_sql::select::{Select, SelectPredicate, TableSource};

pub(super) struct SelectInput {
    pub(super) source: Relation,
    pub(super) distinct: bool,
    pub(super) top: Option<(u32, bool)>,
}

impl Binder<'_> {
    pub(super) fn select_input(
        &mut self,
        select: &Select,
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<SelectInput, QueryError> {
        self.tick(depth)?;
        if select.into.is_some()
            || select.into_external.is_some()
            || select.owner_access
            || (select.external.is_some() && select.from.is_empty())
        {
            return Err(QueryError::Unsupported);
        }
        let (distinct, top) = match select.predicate {
            SelectPredicate::All | SelectPredicate::DistinctRow => (false, None),
            SelectPredicate::Distinct => (true, None),
            SelectPredicate::Top { count, percent }
            | SelectPredicate::DistinctRowTop { count, percent } => (false, Some((count, percent))),
            SelectPredicate::DistinctTop { count, percent } => (true, Some((count, percent))),
        };
        if top.is_some_and(|(n, p)| n == 0 || (p && n > PERCENT_TOTAL)) {
            return Err(QueryError::Unsupported);
        }
        if select.from.len() > MAX_FROM_ITEMS
            || select.projection.is_empty()
            || select.projection.len() > SOURCE_SCHEMA_POLICY.max_columns
            || select.group_by.len() > MAX_GROUP_KEYS
        {
            return Err(QueryError::StructureLimit);
        }
        let namespace = self.external_namespace(select.external.as_ref(), depth + 1)?;
        let source = self.select_source(&select.from, outer, depth, namespace)?;
        Ok(SelectInput {
            source,
            distinct,
            top,
        })
    }

    // A single named source needs none of the cross-product construction frame.
    #[inline(never)]
    fn select_source(
        &mut self,
        from: &[TableSource],
        outer: &[&[Slot]],
        depth: usize,
        namespace: SourceNamespace,
    ) -> Result<Relation, QueryError> {
        match from {
            [] => Ok(Relation {
                slots: Vec::new(),
                node: RelationNode::Unit,
            }),
            [source] => self.relation(source, outer, depth + 1, namespace),
            _ => self.cross_source(from, outer, depth, namespace),
        }
    }

    #[inline(never)]
    fn cross_source(
        &mut self,
        from: &[TableSource],
        outer: &[&[Slot]],
        depth: usize,
        namespace: SourceNamespace,
    ) -> Result<Relation, QueryError> {
        let mut source = Relation {
            slots: vec![],
            node: RelationNode::Unit,
        };
        for item in from {
            let right = self.relation(item, outer, depth + 1, namespace)?;
            if matches!(source.node, RelationNode::Unit) {
                source = right;
                continue;
            }
            let slots = combine_slots(&source.slots, &right.slots)?;
            source = Relation {
                slots,
                node: RelationNode::Cross(Box::new(source), Box::new(right)),
            };
        }
        Ok(source)
    }
}
