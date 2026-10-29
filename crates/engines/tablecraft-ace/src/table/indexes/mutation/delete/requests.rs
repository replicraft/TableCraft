//! Explicit physical-index requests; the complete before check proves omissions.
use crate::{
    EditError, InspectError,
    header::check,
    table::indexes::{NativeIndex, key::KeyProfile, page::KeyRef},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::indexes::mutation) enum Requests<'a> {
    Complete(&'a [u32]),
    Indexed(Vec<u32>),
}

impl Requests<'_> {
    pub(in crate::table::indexes::mutation) fn references(&self) -> &[u32] {
        match self {
            Self::Complete(references) => references,
            Self::Indexed(references) => references,
        }
    }
}

pub(in crate::table::indexes::mutation) fn capture<'a, 'k, P: KeyProfile>(
    index: &NativeIndex,
    profile: &P,
    keys: impl IntoIterator<Item = &'k KeyRef<P::Key>>,
    requested: &'a [u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Requests<'a>, EditError>
where
    P::Key: 'k,
{
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    if !constraints.ignore_nulls() || requested.is_empty() {
        return Ok(Requests::Complete(requested));
    }
    check(budget, cancelled)?;
    budget
        .charge_allocation(requested.len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut effective = Vec::new();
    effective
        .try_reserve_exact(requested.len())
        .map_err(|_| InspectError::Allocation)?;
    let per_key_work = requested
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(profile.components()))
        .ok_or(InspectError::StructureLimit)?;
    for key in keys {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, per_key_work as u64)
            .map_err(InspectError::from)?;
        if profile.constraint_nulls(&key.key)?.0 {
            return Err(InspectError::Malformed.into());
        }
        if requested.contains(&key.row) {
            // Checked complete membership has distinct roots; do not permit an
            // accidental duplicate to grow the paid request vector instead.
            if effective.len() == requested.len() || effective.contains(&key.row) {
                return Err(EditError::Preservation);
            }
            effective.push(key.row);
        }
    }
    Ok(Requests::Indexed(effective))
}
