//! Closed semantic dictionary ordering for explicitly reconstructed field history.
use super::{ALLOW_EMPTY, CreateDatabaseError, DEFAULT_VALUE, DESCRIPTION, REQUIRED};
use crate::{InspectError, TableProperties, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const PROPERTY_COUNT: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::creation) enum PropertyName {
    Required,
    AllowEmpty,
    DefaultValue,
    Description,
}

impl PropertyName {
    fn spelling(self) -> &'static str {
        match self {
            Self::Required => REQUIRED,
            Self::AllowEmpty => ALLOW_EMPTY,
            Self::DefaultValue => DEFAULT_VALUE,
            Self::Description => DESCRIPTION,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::creation) struct Order {
    names: [PropertyName; PROPERTY_COUNT],
    count: usize,
}

impl Order {
    pub(in crate::creation) const fn empty() -> Self {
        Self {
            names: [PropertyName::Required; PROPERTY_COUNT],
            count: 0,
        }
    }

    /// Capture only known dictionary keys after the complete framing proof.
    /// This records order and never qualifies values or the source image.
    pub(in crate::creation) fn capture(
        properties: &TableProperties,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        if properties.raw_bytes().is_empty() {
            check(budget, cancelled)?;
            if !properties.groups().is_empty() {
                return Err(InspectError::Unsupported);
            }
            return Ok(Self::empty());
        }
        let names = crate::properties::framing::ordered_names(properties, budget, cancelled)?;
        let mut order = Self::empty();
        for raw in names {
            check(budget, cancelled)?;
            let mut matched = None;
            for known in [
                PropertyName::Required,
                PropertyName::AllowEmpty,
                PropertyName::DefaultValue,
                PropertyName::Description,
            ] {
                budget.charge(Resource::Steps, raw.len() as u64)?;
                if raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes(*unit))
                    .eq(known.spelling().encode_utf16())
                {
                    matched = Some(known);
                    break;
                }
            }
            let name = matched.ok_or(InspectError::Unsupported)?;
            if order.names().contains(&name) || order.count == PROPERTY_COUNT {
                return Err(InspectError::Unsupported);
            }
            order.names[order.count] = name;
            order.count += 1;
        }
        Ok(order)
    }

    pub(in crate::creation) fn names(&self) -> &[PropertyName] {
        &self.names[..self.count]
    }

    /// Preserve existing indices and append only newly required known keys.
    /// A first Number ADD has the independently observed Default/Required order.
    pub(in crate::creation) fn extend(
        mut self,
        required: bool,
        allow_empty: bool,
        default_value: bool,
        description: bool,
        first_number: bool,
    ) -> Result<Self, CreateDatabaseError> {
        let additions = if self.count == 0 && first_number {
            [
                PropertyName::DefaultValue,
                PropertyName::Required,
                PropertyName::AllowEmpty,
                PropertyName::Description,
            ]
        } else {
            [
                PropertyName::Required,
                PropertyName::AllowEmpty,
                PropertyName::DefaultValue,
                PropertyName::Description,
            ]
        };
        for name in additions {
            if needed(name, required, allow_empty, default_value, description)
                && !self.names().contains(&name)
            {
                if self.count == PROPERTY_COUNT {
                    return Err(CreateDatabaseError::Declaration);
                }
                self.names[self.count] = name;
                self.count += 1;
            }
        }
        self.require_exact(required, allow_empty, default_value, description)?;
        Ok(self)
    }

    pub(super) fn require_exact(
        self,
        required: bool,
        allow_empty: bool,
        default_value: bool,
        description: bool,
    ) -> Result<(), CreateDatabaseError> {
        if self.count
            != usize::from(required)
                + usize::from(allow_empty)
                + usize::from(default_value)
                + usize::from(description)
            || self
                .names()
                .iter()
                .any(|name| !needed(*name, required, allow_empty, default_value, description))
        {
            return Err(CreateDatabaseError::Declaration);
        }
        Ok(())
    }
}

fn needed(
    name: PropertyName,
    required: bool,
    allow_empty: bool,
    default_value: bool,
    description: bool,
) -> bool {
    match name {
        PropertyName::Required => required,
        PropertyName::AllowEmpty => allow_empty,
        PropertyName::DefaultValue => default_value,
        PropertyName::Description => description,
    }
}
