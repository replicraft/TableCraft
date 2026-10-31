//! Exact bounded native property selectors shared by explicit guard commands.
use std::fmt;
use tablecraft_ace::{BeforeEventGuardSelector, InspectError, NativeImage};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Debug)]
pub(in crate::commands) enum Error {
    Native(InspectError),
    Selection,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(error) => error.fmt(formatter),
            Self::Selection => {
                formatter.write_str("expected one or more bounded --guard-property KIND NAME pairs")
            }
        }
    }
}

pub(super) fn selectors<'a>(
    values: &'a [String],
    budget: &mut ResourceBudget,
) -> Result<Vec<BeforeEventGuardSelector<'a>>, Error> {
    if values.is_empty()
        || !values.len().is_multiple_of(2)
        || values.len() / 2 > NativeImage::MAX_BEFORE_GUARD_PROPERTIES
    {
        return Err(Error::Selection);
    }
    budget
        .charge_allocation(
            (values.len() / 2) as u64,
            size_of::<BeforeEventGuardSelector<'_>>() as u64,
        )
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(values.len() / 2)
        .map_err(|_| Error::Native(InspectError::Allocation))?;
    for pair in values.as_chunks::<2>().0 {
        budget
            .charge(Resource::Steps, (pair[0].len() + pair[1].len()) as u64)
            .map_err(InspectError::from)
            .map_err(Error::Native)?;
        let kind = pair[0].parse::<u16>().map_err(|_| Error::Selection)?;
        if pair[1].is_empty() {
            return Err(Error::Selection);
        }
        selected.push(BeforeEventGuardSelector::new(kind, &pair[1]));
    }
    Ok(selected)
}
