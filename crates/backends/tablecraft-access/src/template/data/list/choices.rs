//! Inert single-Choice option metadata; stored cells are never derived from it.
use super::fields::requests_namespace;
use crate::template::{TemplateError, check, tree::Node};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn validate(
    node: &Node,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TemplateError> {
    node.whitespace()?;
    if !node.attributes.is_empty() {
        return Err(TemplateError::Unsupported);
    }
    for choice in &node.children {
        check(budget, cancelled)?;
        if !requests_namespace(choice)
            || choice.name.local != "CHOICE"
            || !choice.children.is_empty()
        {
            return Err(TemplateError::Unsupported);
        }
        budget
            .charge(Resource::Steps, choice.text.len() as u64)
            .map_err(InspectError::from)?;
        for attribute in &choice.attributes {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (attribute.name.local.len() + attribute.value.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if !attribute.name.namespace.is_empty()
                || !matches!(attribute.name.local.as_str(), "Value" | "JumpTo")
            {
                return Err(TemplateError::Unsupported);
            }
        }
        // Literal body, display/resource and survey-navigation strings remain
        // exactly the inert metadata already retained by the bounded XML tree.
    }
    Ok(())
}
