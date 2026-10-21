//! Hidden members align by child identity; independent collections form products.
use super::{super::Binder, displayed::collection, item::ItemBinding};
use crate::{
    QueryError,
    plan::{Expr, ParentMember, ParentPredicate, ParentProduct, Relation, Slot},
};

impl Binder<'_> {
    pub(super) fn parent_product(
        &mut self,
        source: &Relation,
        mut predicate: Expr,
        used: &[bool],
        virtual_fields: &[(usize, usize)],
        depth: usize,
    ) -> Result<ParentPredicate, QueryError> {
        self.tick(depth)?;
        let width = virtual_fields
            .iter()
            .filter(|(field, _)| used[*field])
            .count();
        self.charge(
            width * (size_of::<ParentMember>() + size_of::<&Slot>()) + size_of::<ParentProduct>(),
        )?;
        let mut groups: Vec<&Slot> = Vec::new();
        let mut members = Vec::new();
        groups
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        members
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        let mut capture = false;
        for (field, key) in virtual_fields.iter().filter(|(field, _)| used[*field]) {
            self.tick(depth)?;
            let slot = source
                .slots
                .iter()
                .filter(|slot| slot.available)
                .nth(*field)
                .ok_or(QueryError::Source)?;
            let mut matched = None;
            for (index, other) in groups.iter().enumerate() {
                self.tick(depth)?;
                if collection(slot, other).is_eq() {
                    matched = Some(index);
                    break;
                }
            }
            let group = if let Some(index) = matched {
                index
            } else {
                groups.push(slot);
                groups.len() - 1
            };
            self.replace_item(
                &mut predicate,
                &ItemBinding {
                    field: *field,
                    index: members.len(),
                    width,
                },
                0,
                &mut capture,
                depth + 1,
            )?;
            members.push(ParentMember {
                collection: *key,
                group,
            });
        }
        Ok(ParentPredicate::AnyMany(Box::new(ParentProduct {
            members,
            groups: groups.len(),
            predicate,
            capture,
        })))
    }
}
