//! Borrowed identities of displayed collections, independent of member spelling.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Relation, Slot},
};
use std::cmp::Ordering;

fn ascii(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
}
fn collection(left: &Slot, right: &Slot) -> Ordering {
    let qualifier = match (left.qualifier.as_deref(), right.qualifier.as_deref()) {
        (Some(left), Some(right)) => ascii(left, right),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
    };
    qualifier.then_with(|| ascii(&left.name, &right.name))
}

pub(in crate::binding) struct Displayed<'a> {
    collections: Vec<&'a Slot>,
}
impl<'a> Displayed<'a> {
    pub(in crate::binding) fn new(
        source: &'a Relation,
        shown: &[bool],
        binder: &mut Binder<'_>,
    ) -> Result<Self, QueryError> {
        let slots = || {
            source
                .slots
                .iter()
                .filter(|slot| slot.available)
                .enumerate()
                .filter(|(position, slot)| slot.member.is_some() && shown[*position])
                .map(|(_, slot)| slot)
        };
        let count = slots().count();
        binder.charge(count * size_of::<&Slot>())?;
        let mut collections = Vec::new();
        collections
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        collections.extend(slots());
        // Existing schema/name/source ceilings bound this work. Unstable sorting
        // needs no scratch allocation; binary search avoids a quadratic scan for
        // each hidden field. ASCII comparison matches the binder's name policy.
        collections.sort_unstable_by(|left, right| collection(left, right));
        collections.dedup_by(|left, right| collection(left, right).is_eq());
        Ok(Self { collections })
    }
    pub(in crate::binding) fn contains(&self, slot: &Slot) -> bool {
        self.collections
            .binary_search_by(|shown| collection(shown, slot))
            .is_ok()
    }
}
