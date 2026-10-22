//! Parse stored SOAP metadata only; this module has no client/provider capability.
use super::super::super::tree::Node;
use super::{
    TemplateError, check,
    fields::{self, Field},
};
use tablecraft_core::limits::ResourceBudget;

pub(super) const SHAREPOINT: &str = "http://schemas.microsoft.com/sharepoint/soap/";
const SOAP: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const MAX_REQUESTS: usize = 128;
const PROPERTIES: u8 = 1;
const NEW_FIELDS: u8 = 1 << 1;
const UPDATE_FIELDS: u8 = 1 << 2;
const REQUIRED_FIELDS: u8 = PROPERTIES | NEW_FIELDS | UPDATE_FIELDS;

pub(super) fn read(
    root: &Node,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Field>, TemplateError> {
    root.whitespace()?;
    root.only_attributes(&[])?;
    if !root.is("", "Root") || root.children.is_empty() || root.children.len() > MAX_REQUESTS {
        return Err(TemplateError::Unsupported);
    }
    let mut created = false;
    let mut fields = Vec::new();
    for envelope in &root.children {
        check(budget, cancelled)?;
        envelope.whitespace()?;
        envelope.only_attributes(&[])?;
        if !envelope.is(SOAP, "Envelope") || envelope.children.len() != 1 {
            return Err(TemplateError::Unsupported);
        }
        let body = &envelope.children[0];
        body.whitespace()?;
        body.only_attributes(&[])?;
        if !body.is(SOAP, "Body") || body.children.len() != 1 {
            return Err(TemplateError::Malformed);
        }
        let request = &body.children[0];
        request.whitespace()?;
        request.only_attributes(&[])?;
        if request.name.namespace != SHAREPOINT {
            return Err(TemplateError::Unsupported);
        }
        match request.name.local.as_str() {
            "AddList" => {
                if created {
                    return Err(TemplateError::Unsupported);
                }
                let names: Vec<_> = request
                    .children
                    .iter()
                    .filter(|child| child.is(SHAREPOINT, "listName"))
                    .collect();
                let ids: Vec<_> = request
                    .children
                    .iter()
                    .filter(|child| child.is(SHAREPOINT, "templateID"))
                    .collect();
                if names.len() != 1
                    || ids.len() != 1
                    || !names[0].scalar()?.eq_ignore_ascii_case(name)
                {
                    return Err(TemplateError::Malformed);
                }
                let identifier = ids[0].scalar()?;
                if identifier.is_empty()
                    || !identifier.bytes().all(|b| b.is_ascii_digit())
                    || identifier.parse::<i32>().is_err()
                {
                    return Err(TemplateError::Unsupported);
                }
                created = true;
            }
            "UpdateList" if created => {
                let expected = [
                    "listName",
                    "listProperties",
                    "newFields",
                    "updateFields",
                    "deleteFields",
                    "listVersion",
                ];
                if request.children.len() < REQUIRED_FIELDS.count_ones() as usize
                    || request.children.len() > expected.len()
                {
                    return Err(TemplateError::Malformed);
                }
                let mut last = None;
                let mut mandatory = 0;
                for child in &request.children {
                    check(budget, cancelled)?;
                    if child.name.namespace != SHAREPOINT {
                        return Err(TemplateError::Unsupported);
                    }
                    let position = expected
                        .iter()
                        .position(|name| *name == child.name.local)
                        .ok_or(TemplateError::Unsupported)?;
                    if last.is_some_and(|last| position <= last) {
                        return Err(TemplateError::Malformed);
                    }
                    last = Some(position);
                    match child.name.local.as_str() {
                        "listName" => {
                            if child.scalar()?.is_empty() {
                                return Err(TemplateError::Malformed);
                            }
                        }
                        "listProperties" => {
                            mandatory |= PROPERTIES;
                            child.only_attributes(&[])?;
                            child.whitespace()?;
                            if child.children.len() > 1
                                || child.children.first().is_some_and(|node| {
                                    !fields::requests_namespace(node) || node.name.local != "List"
                                })
                            {
                                return Err(TemplateError::Malformed);
                            }
                        }
                        "newFields" => {
                            mandatory |= NEW_FIELDS;
                            fields::apply(&mut fields, child, "newFields", budget, cancelled)?;
                        }
                        "updateFields" => {
                            mandatory |= UPDATE_FIELDS;
                            fields::apply(&mut fields, child, "updateFields", budget, cancelled)?;
                        }
                        "deleteFields" => {
                            fields::apply(&mut fields, child, "deleteFields", budget, cancelled)?
                        }
                        "listVersion" => {
                            let version = child.scalar()?;
                            if version.is_empty() || !version.bytes().all(|b| b.is_ascii_digit()) {
                                return Err(TemplateError::Malformed);
                            }
                        }
                        _ => return Err(TemplateError::Unsupported),
                    }
                }
                if mandatory != REQUIRED_FIELDS {
                    return Err(TemplateError::Malformed);
                }
            }
            _ => return Err(TemplateError::Unsupported),
        }
    }
    if !created || fields.is_empty() {
        return Err(TemplateError::Unsupported);
    }
    Ok(fields)
}
