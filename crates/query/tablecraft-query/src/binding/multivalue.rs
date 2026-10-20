mod criteria;
use crate::{QueryError, plan::Slot, profile::SOURCE_SCHEMA_POLICY};

pub(super) fn resolve_member(
    qualifier: Option<&str>,
    name: &str,
    member: &str,
    slots: &[Slot],
) -> Result<Option<usize>, QueryError> {
    let mut found = None;
    let mut index = 0;
    for slot in slots {
        if slot
            .member
            .as_deref()
            .is_some_and(|name| SOURCE_SCHEMA_POLICY.names.equals(name, member))
            && SOURCE_SCHEMA_POLICY.names.equals(&slot.name, name)
            && qualifier.is_none_or(|q| {
                slot.qualifier
                    .as_deref()
                    .is_some_and(|s| SOURCE_SCHEMA_POLICY.names.equals(q, s))
            })
        {
            if found.is_some() {
                return Err(QueryError::AmbiguousName);
            }
            found = Some(index);
        }
        index += usize::from(slot.available);
    }
    Ok(found)
}
