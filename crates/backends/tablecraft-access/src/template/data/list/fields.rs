//! Ordered CAML field declaration updates; no formula/default evaluation.
use super::super::super::check;
use super::super::super::tree::Node;
use super::super::{MAX_COLUMNS, TemplateError};
use std::collections::BTreeSet;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;
use uuid::Uuid;

const MAX_FIELDS: usize = MAX_COLUMNS;
const MAX_NAME_BYTES: usize = 1024;
const MAX_TYPE_BYTES: usize = 64;
const IDENTITY_ATTRIBUTES: [&str; 3] = ["ID", "Name", "DisplayName"];

#[derive(Default)]
pub(super) struct Field {
    id: Option<Uuid>,
    name: Option<String>,
    display: Option<String>,
    pub kind: Option<String>,
    pub max_length: Option<usize>,
    pub required: Option<bool>,
    pub calculated: bool,
}
impl Field {
    pub fn matches_column(&self, display: &str, xml_name: &str) -> bool {
        self.display
            .as_ref()
            .is_some_and(|value| value.eq_ignore_ascii_case(display))
            || self.name.as_ref().is_some_and(|value| {
                value.eq_ignore_ascii_case(xml_name) || value.eq_ignore_ascii_case(display)
            })
    }
    fn matches(&self, node: &Node) -> Result<bool, TemplateError> {
        let id = node
            .attribute("ID")
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| TemplateError::Malformed)?;
        Ok(self.id.zip(id).is_some_and(|(old, new)| old == new)
            || self
                .name
                .as_deref()
                .zip(node.attribute("Name"))
                .is_some_and(|(old, new)| old.eq_ignore_ascii_case(new))
            || self
                .display
                .as_deref()
                .zip(node.attribute("DisplayName"))
                .is_some_and(|(old, new)| old.eq_ignore_ascii_case(new)))
    }
    fn apply(&mut self, node: &Node) -> Result<(), TemplateError> {
        if node
            .attributes
            .iter()
            .any(|attribute| !attribute.name.namespace.is_empty())
        {
            return Err(TemplateError::Unsupported);
        }
        if let Some(id) = node.attribute("ID") {
            let id = Uuid::parse_str(id).map_err(|_| TemplateError::Malformed)?;
            if self.id.is_some_and(|old| old != id) {
                return Err(TemplateError::Malformed);
            }
            self.id = Some(id);
        }
        for (slot, key) in [
            (&mut self.name, "Name"),
            (&mut self.display, "DisplayName"),
            (&mut self.kind, "Type"),
        ] {
            if let Some(value) = node.attribute(key) {
                let maximum = if key == "Type" {
                    MAX_TYPE_BYTES
                } else {
                    MAX_NAME_BYTES
                };
                if value.len() > maximum {
                    return Err(TemplateError::Unsupported);
                }
                if key == "Name"
                    && slot
                        .as_ref()
                        .is_some_and(|old| !old.eq_ignore_ascii_case(value))
                {
                    return Err(TemplateError::Unsupported);
                }
                if value.is_empty() || value.chars().any(char::is_control) {
                    return Err(TemplateError::Malformed);
                }
                *slot = Some(value.into());
            }
        }
        if let Some(maximum) = node.attribute("MaxLength") {
            if maximum.is_empty() || !maximum.bytes().all(|b| b.is_ascii_digit()) {
                return Err(TemplateError::Malformed);
            }
            let maximum = maximum.parse().map_err(|_| TemplateError::Unsupported)?;
            if maximum == 0 {
                return Err(TemplateError::Malformed);
            }
            self.max_length = Some(maximum);
        }
        if let Some(required) = boolean(node, "Required")? {
            self.required = Some(required);
        }
        if let Some(calculated) = boolean(node, "Calculated")? {
            self.calculated = calculated;
        }
        if self.kind.as_deref() == Some("Calculated") {
            self.calculated = true;
        }
        // Known presentation/default/validation children remain inert. Unknown
        // construction children are refused instead of being treated as commands.
        node.whitespace()?;
        for child in &node.children {
            if !requests_namespace(child)
                || !matches!(
                    child.name.local.as_str(),
                    "ValidationDisplayNames"
                        | "Default"
                        | "DefaultFormula"
                        | "ValidationFormula"
                        | "ValidationMessage"
                )
            {
                return Err(TemplateError::Unsupported);
            }
        }
        Ok(())
    }
}

pub(super) fn apply(
    fields: &mut Vec<Field>,
    container: &Node,
    action: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TemplateError> {
    container.whitespace()?;
    container.only_attributes(&[])?;
    if container.children.is_empty() {
        return Ok(());
    }
    if container.children.len() != 1 || !is(&container.children[0], "Fields") {
        return Err(TemplateError::Malformed);
    }
    let methods = &container.children[0];
    methods.whitespace()?;
    methods.only_attributes(&[])?;
    let mut ids = BTreeSet::new();
    for method in &methods.children {
        check(budget, cancelled)?;
        method.whitespace()?;
        method.only_attributes(&["ID", "AddToView"])?;
        if !is(method, "Method") || method.children.len() != 1 || !is(&method.children[0], "Field")
        {
            return Err(TemplateError::Malformed);
        }
        if let Some(id) = method.attribute("ID")
            && (id.is_empty() || !ids.insert(id))
        {
            return Err(TemplateError::Malformed);
        }
        let node = &method.children[0];
        if !IDENTITY_ATTRIBUTES
            .iter()
            .any(|key| node.attribute(key).is_some())
        {
            return Err(TemplateError::Malformed);
        }
        let mut matching = Vec::new();
        for (index, field) in fields.iter().enumerate() {
            check(budget, cancelled)?;
            if field.matches(node)? {
                matching.push(index);
            }
        }
        if matching.len() > 1 {
            return Err(TemplateError::Malformed);
        }
        let changed = match (action, matching.first().copied()) {
            ("newFields", Some(_)) => return Err(TemplateError::Malformed),
            ("deleteFields", Some(index)) => {
                node.only_attributes(&IDENTITY_ATTRIBUTES)?;
                node.whitespace()?;
                if !node.children.is_empty() {
                    return Err(TemplateError::Unsupported);
                }
                fields.remove(index);
                None
            }
            ("deleteFields", None) => return Err(TemplateError::Unsupported),
            ("updateFields", Some(index)) => {
                fields[index].apply(node)?;
                Some(index)
            }
            ("newFields" | "updateFields", None) => {
                // Base fields may first appear as a full update declaration.
                // No type is synthesized from templateID or the sample records.
                if fields.len() >= MAX_FIELDS
                    || node.attribute("Type").is_none()
                    || node.attribute("Name").is_none() && node.attribute("DisplayName").is_none()
                {
                    return Err(TemplateError::Unsupported);
                }
                let mut field = Field::default();
                field.apply(node)?;
                fields
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                fields.push(field);
                Some(fields.len() - 1)
            }
            _ => return Err(TemplateError::Malformed),
        };
        // Only the changed declaration can introduce a new identity collision.
        if let Some(index) = changed {
            let field = &fields[index];
            for (other_index, other) in fields.iter().enumerate() {
                check(budget, cancelled)?;
                if index != other_index
                    && (field.id.zip(other.id).is_some_and(|(a, b)| a == b)
                        || field
                            .name
                            .as_ref()
                            .zip(other.name.as_ref())
                            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
                        || field
                            .display
                            .as_ref()
                            .zip(other.display.as_ref())
                            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)))
                {
                    return Err(TemplateError::Malformed);
                }
            }
        }
    }
    Ok(())
}
fn boolean(node: &Node, key: &str) -> Result<Option<bool>, TemplateError> {
    node.attribute(key)
        .map(|value| match value {
            "TRUE" => Ok(true),
            "FALSE" => Ok(false),
            _ => Err(TemplateError::Malformed),
        })
        .transpose()
}
pub(super) fn requests_namespace(node: &Node) -> bool {
    node.name.namespace.is_empty() || node.name.namespace == super::requests::SHAREPOINT
}
fn is(node: &Node, local: &str) -> bool {
    requests_namespace(node) && node.name.local == local
}
