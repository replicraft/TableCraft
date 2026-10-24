//! Snapshot-local key validation shared by full and metadata-only collections.
use super::{ComplexReferenceViolation, metadata::ComplexParentLink};
use crate::{InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table::complex) struct Parents {
    parents: Vec<ComplexParentLink>,
    keys: Vec<(i32, usize)>,
}

pub(in crate::table::complex) struct Membership {
    pub(super) parents: Vec<ComplexParentLink>,
    pub(super) order: Vec<(usize, usize)>,
}

impl Membership {
    pub(in crate::table::complex) fn parents(&self) -> &[ComplexParentLink] {
        &self.parents
    }

    pub(in crate::table::complex) fn child_ordinal(
        &self,
        parent: usize,
        child: usize,
    ) -> Option<usize> {
        let range = &self.parents.get(parent)?.range;
        if child >= range.len() {
            return None;
        }

        self.order
            .get(range.start.checked_add(child)?)
            .map(|(_, ordinal)| *ordinal)
    }
}

impl Parents {
    pub(in crate::table::complex) fn read(
        rows: Vec<Vec<Value>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let mut parents = reserved::<ComplexParentLink>(rows.len(), budget)?;
        let mut keys = reserved::<(i32, usize)>(rows.len(), budget)?;
        for (ordinal, row) in rows.into_iter().enumerate() {
            check(budget, cancelled)?;
            let key = match row.as_slice() {
                [Value::Int32(key)] if *key > 0 => {
                    keys.push((*key, ordinal));
                    Some(*key)
                }
                [Value::Int32(_)] => return Err(InspectError::Unsupported),
                [Value::Null] => None,
                _ => return Err(InspectError::Malformed),
            };
            parents.push(ComplexParentLink { key, range: 0..0 });
        }

        sort(&mut keys, budget, cancelled)?;
        for pair in keys.windows(2) {
            check(budget, cancelled)?;
            if pair[0].0 == pair[1].0 {
                return Err(ComplexReferenceViolation::DuplicateParent.into());
            }
        }

        Ok(Self { parents, keys })
    }

    pub(in crate::table::complex) fn link(
        mut self,
        rows: &[Vec<Value>],
        foreign_key: usize,
        identity: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Membership, InspectError> {
        let mut identities = reserved::<i32>(rows.len(), budget)?;
        let mut order = reserved::<(usize, usize)>(rows.len(), budget)?;
        let search_steps = u64::from(self.keys.len().checked_ilog2().unwrap_or(0)) + 1;
        for (ordinal, row) in rows.iter().enumerate() {
            check(budget, cancelled)?;
            let key = child_key(row.get(foreign_key))?;
            identities.push(child_key(row.get(identity))?);
            budget.charge(Resource::Steps, search_steps)?;
            let parent = self
                .keys
                .binary_search_by_key(&key, |(key, _)| *key)
                .map_err(|_| ComplexReferenceViolation::MissingParent)?;
            order.push((self.keys[parent].1, ordinal));
        }

        sort(&mut identities, budget, cancelled)?;
        for pair in identities.windows(2) {
            check(budget, cancelled)?;
            if pair[0] == pair[1] {
                return Err(ComplexReferenceViolation::DuplicateChild.into());
            }
        }

        sort(&mut order, budget, cancelled)?;
        let mut end = 0;
        for (ordinal, parent) in self.parents.iter_mut().enumerate() {
            check(budget, cancelled)?;
            let start = end;
            while order.get(end).is_some_and(|(owner, _)| *owner == ordinal) {
                check(budget, cancelled)?;
                end += 1;
            }
            parent.range = start..end;
        }
        if end != order.len() {
            return Err(InspectError::Malformed);
        }

        check(budget, cancelled)?;
        Ok(Membership {
            parents: self.parents,
            order,
        })
    }
}

fn child_key(value: Option<&Value>) -> Result<i32, InspectError> {
    match value {
        Some(Value::Int32(key)) if *key > 0 => Ok(*key),
        Some(Value::Int32(_)) => Err(InspectError::Unsupported),
        Some(Value::Null) => Err(ComplexReferenceViolation::NullChild.into()),
        _ => Err(InspectError::Malformed),
    }
}

fn reserved<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, InspectError> {
    budget.charge_allocation(count as u64, size_of::<T>() as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}

fn sort<T: Ord>(
    values: &mut [T],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    let levels = u64::from(values.len().checked_ilog2().unwrap_or(0)) + 1;
    budget.charge(
        Resource::Steps,
        values.len() as u64 * levels * size_of::<T>() as u64,
    )?;
    values.sort_unstable();
    check(budget, cancelled)
}
