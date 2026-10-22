//! Inert List Definition declaration state and agreement with stored inline XSD.
mod fields;
mod requests;

use super::super::{TemplateObject, TemplatePackage, check, tree};
use super::{
    TemplateError,
    schema::{Kind, Schema},
};
use tablecraft_core::limits::ResourceBudget;

const PROPERTIES_RELATIONSHIP: &str =
    "http://schemas.microsoft.com/office/2007/relationships/ObjectProperties";
const PROPERTIES_CONTENT_TYPE: &str = "application/vnd-ms-access.objectproperties+xml";

pub(super) fn validate(
    package: &TemplatePackage,
    object: &TemplateObject,
    schema: &Schema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TemplateError> {
    if !object.is_list_definition() {
        return Ok(());
    }
    let _span = tracing::debug_span!("access.template.list.agreement").entered();
    let part = package
        .part(object.part())
        .ok_or(TemplateError::Malformed)?;
    if !part.content_type().eq_ignore_ascii_case("application/xml") {
        return Err(TemplateError::Malformed);
    }
    let mut properties = part
        .relationships()
        .iter()
        .filter(|r| r.kind() == PROPERTIES_RELATIONSHIP);
    let property = properties
        .next()
        .and_then(|relationship| package.part(relationship.target()))
        .ok_or(TemplateError::Malformed)?;
    if properties.next().is_some()
        || !property
            .content_type()
            .eq_ignore_ascii_case(PROPERTIES_CONTENT_TYPE)
    {
        return Err(TemplateError::Malformed);
    }
    // Properties remain inert; their application schema is not run or inferred.
    let root = tree::parse(part.bytes(), budget, cancelled)?;
    let fields = requests::read(&root, object.name(), budget, cancelled)?;
    let mut used = vec![false; fields.len()];
    for (column, stored) in schema.columns.iter().zip(&schema.fields) {
        check(budget, cancelled)?;
        let matching: Vec<_> = fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.matches_column(column.name(), &stored.xml_name))
            .collect();
        if matching.len() != 1 {
            return Err(TemplateError::Malformed);
        }
        let (index, field) = matching[0];
        if used[index] {
            return Err(TemplateError::Malformed);
        }
        used[index] = true;
        let kind = field.kind.as_deref().ok_or(TemplateError::Unsupported)?;
        let type_matches = matches!(
            (kind, stored.kind),
            ("Text" | "Note", Kind::String)
                | ("Boolean", Kind::Boolean)
                | ("Counter" | "Integer", Kind::Int)
                | ("Number", Kind::Double)
                | ("Currency", Kind::Decimal)
                | ("DateTime", Kind::DateTime)
        );
        if field.calculated
            || !matches!(
                kind,
                "Text"
                    | "Note"
                    | "Boolean"
                    | "Counter"
                    | "Integer"
                    | "Number"
                    | "Currency"
                    | "DateTime"
            )
        {
            return Err(TemplateError::Unsupported);
        }
        if !type_matches {
            return Err(TemplateError::Malformed);
        }
        if field
            .max_length
            .is_some_and(|length| stored.max_length != Some(length))
            || field.required == Some(true) && column.nullable()
            || kind == "Counter" && column.nullable()
        {
            return Err(TemplateError::Malformed);
        }
    }
    // An undeclared/default field or an additional final field cannot disappear
    // from the relation. Hidden is a presentation flag, not SQL permission to omit.
    if used.iter().any(|used| !used) {
        return Err(TemplateError::Unsupported);
    }
    check(budget, cancelled)
}

#[cfg(test)]
mod tests;
