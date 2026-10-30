use super::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeGuardSelectionError,
    BeforeGuardUnsupported,
};
use crate::{InspectError, NativeImage, TableExtra, TableExtraProperty, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Selected<'a> {
    pub group: usize,
    pub property: usize,
    pub value: &'a TableExtraProperty,
}

pub(super) fn validate(
    selectors: &[BeforeEventGuardSelector<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    check(budget, cancelled)?;
    if selectors.is_empty() {
        return Err(BeforeEventGuardError::Selection(
            BeforeGuardSelectionError::Empty,
        ));
    }
    if selectors.len() > NativeImage::MAX_BEFORE_GUARD_PROPERTIES {
        return Err(InspectError::StructureLimit.into());
    }
    for (index, selector) in selectors.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, selector.name().len() as u64)
            .map_err(InspectError::from)?;
        for previous in &selectors[..index] {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    selector.name().len().max(previous.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if previous.group() == selector.group() && previous.name() == selector.name() {
                return Err(BeforeEventGuardError::Selection(
                    BeforeGuardSelectionError::Duplicate,
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn resolve<'a>(
    extra: &'a TableExtra,
    selector: BeforeEventGuardSelector<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Selected<'a>, BeforeEventGuardError> {
    let mut found = None;
    for (group_index, group) in extra.groups().iter().enumerate() {
        check(budget, cancelled)?;
        if group.kind() != selector.group() {
            continue;
        }
        let properties = group
            .properties()
            .ok_or(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Structure,
            ))?;
        for (property_index, property) in properties.iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    property.name().len().max(selector.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if property.name() != selector.name() {
                continue;
            }
            if found.is_some() {
                return Err(BeforeEventGuardError::Selection(
                    BeforeGuardSelectionError::Ambiguous,
                ));
            }
            found = Some(Selected {
                group: group_index,
                property: property_index,
                value: property,
            });
        }
    }
    found.ok_or(BeforeEventGuardError::Selection(
        BeforeGuardSelectionError::Missing,
    ))
}
