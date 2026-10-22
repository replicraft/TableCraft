//! Qualified inline XSD shapes and exact scalar domains, separate from values.
use super::super::tree::{Node, OD, XSD};
use super::{MAX_COLUMNS, TemplateError, check, names};
use std::collections::BTreeSet;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{Column, NameComparison, ScalarType, TypeDescriptor, validate_columns};

pub(super) struct Schema {
    pub root: String,
    pub row: String,
    pub columns: Vec<Column>,
    pub fields: Vec<Field>,
    pub generated: bool,
}
pub(super) struct Field {
    pub xml_name: String,
    pub kind: Kind,
    pub optional: bool,
    pub nillable: bool,
    pub max_length: Option<usize>,
    pub total_digits: Option<usize>,
    pub fraction_digits: Option<usize>,
}
#[derive(Clone, Copy)]
pub(super) enum Kind {
    String,
    Boolean,
    SignedByte,
    UnsignedByte,
    Short,
    UnsignedShort,
    Int,
    UnsignedInt,
    Long,
    Float,
    Double,
    Decimal,
    DateTime,
    Base64,
    Hex,
}
impl Kind {
    fn from_name(name: &str) -> Result<Self, TemplateError> {
        Ok(match name {
            "string" => Self::String,
            "boolean" => Self::Boolean,
            "byte" => Self::SignedByte,
            "unsignedByte" => Self::UnsignedByte,
            "short" => Self::Short,
            "unsignedShort" => Self::UnsignedShort,
            "int" => Self::Int,
            "unsignedInt" => Self::UnsignedInt,
            "long" => Self::Long,
            "float" => Self::Float,
            "double" => Self::Double,
            "decimal" => Self::Decimal,
            "dateTime" => Self::DateTime,
            "base64Binary" => Self::Base64,
            "hexBinary" => Self::Hex,
            _ => return Err(TemplateError::Unsupported),
        })
    }
    fn scalar(self) -> ScalarType {
        match self {
            Self::String => ScalarType::Text,
            Self::Boolean => ScalarType::Boolean,
            Self::SignedByte | Self::Short => ScalarType::Int16,
            Self::UnsignedByte => ScalarType::Byte,
            Self::UnsignedShort | Self::Int => ScalarType::Int32,
            Self::UnsignedInt | Self::Long => ScalarType::Int64,
            Self::Float => ScalarType::Single,
            Self::Double => ScalarType::Double,
            Self::Decimal => ScalarType::Decimal,
            Self::DateTime => ScalarType::DateTime,
            Self::Base64 | Self::Hex => ScalarType::Binary,
        }
    }
}

pub(super) fn read(
    node: &Node,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Schema, TemplateError> {
    if !node.is(XSD, "schema") {
        return Err(TemplateError::Malformed);
    }
    node.only_attributes(&[
        "elementFormDefault",
        "attributeFormDefault",
        "targetNamespace",
        "version",
        "id",
    ])?;
    if node
        .attribute("targetNamespace")
        .is_some_and(|value| !value.is_empty())
        || node
            .attribute("elementFormDefault")
            .is_some_and(|value| value != "unqualified")
        || node
            .attribute("attributeFormDefault")
            .is_some_and(|value| value != "unqualified")
    {
        return Err(TemplateError::Unsupported);
    }
    let declarations = children(node)?;
    if declarations.len() != 2 || declarations.iter().any(|node| !node.is(XSD, "element")) {
        return Err(TemplateError::Unsupported);
    }
    let mut row = None;
    let mut root = None;
    for node in declarations {
        node.only_attributes(&["name"])?;
        let xml_name = xml_name(node.attribute("name").ok_or(TemplateError::Malformed)?)?;
        if names::decode(xml_name)? == table {
            if row.replace(node).is_some() {
                return Err(TemplateError::Malformed);
            }
        } else if root.replace(node).is_some() {
            return Err(TemplateError::Unsupported);
        }
    }
    let row = row.ok_or(TemplateError::Malformed)?;
    let root = root.ok_or(TemplateError::Malformed)?;
    let row_name = row.attribute("name").ok_or(TemplateError::Malformed)?;
    let root_complex = single(root, "complexType")?;
    root_complex.only_attributes(&[])?;
    let root_children = children(root_complex)?;
    let sequence = root_children.first().ok_or(TemplateError::Malformed)?;
    if !sequence.is(XSD, "sequence") || root_children.len() > 2 {
        return Err(TemplateError::Unsupported);
    }
    sequence.only_attributes(&[])?;
    let references = children(sequence)?;
    if references.len() != 1 || !references[0].is(XSD, "element") {
        return Err(TemplateError::Unsupported);
    }
    let reference = references[0];
    reference.only_attributes(&["ref", "minOccurs", "maxOccurs"])?;
    if reference
        .qualified("ref")
        .is_none_or(|name| !name.namespace.is_empty() || name.local != row_name)
        || reference.attribute("minOccurs").unwrap_or("1") != "0"
        || reference.attribute("maxOccurs") != Some("unbounded")
        || !children(reference)?.is_empty()
    {
        return Err(TemplateError::Unsupported);
    }
    if let Some(generated) = root_children.get(1) {
        generated.only_attributes(&["name", "type", "use"])?;
        if !generated.is(XSD, "attribute")
            || generated.attribute("name") != Some("generated")
            || generated
                .attribute("use")
                .is_some_and(|value| value != "optional")
            || generated
                .qualified("type")
                .is_none_or(|name| name.namespace != XSD || name.local != "dateTime")
            || !children(generated)?.is_empty()
        {
            return Err(TemplateError::Unsupported);
        }
    }
    let row_complex = single(row, "complexType")?;
    row_complex.only_attributes(&[])?;
    let sequence = single(row_complex, "sequence")?;
    sequence.only_attributes(&[])?;
    let mut columns = Vec::new();
    let mut fields = Vec::new();
    let mut names_seen = BTreeSet::new();
    for node in children(sequence)? {
        check(budget, cancelled)?;
        if fields.len() >= MAX_COLUMNS || !node.is(XSD, "element") {
            return Err(TemplateError::Unsupported);
        }
        node.only_attributes(&["name", "type", "minOccurs", "maxOccurs", "nillable"])?;
        if node
            .attribute("maxOccurs")
            .is_some_and(|value| value != "1")
        {
            return Err(TemplateError::Unsupported);
        }
        let xml_name = xml_name(node.attribute("name").ok_or(TemplateError::Malformed)?)?;
        let name = names::decode(xml_name)?;
        if !names_seen.insert(xml_name) {
            return Err(TemplateError::Malformed);
        }
        let optional = match node.attribute("minOccurs").unwrap_or("1") {
            "0" => true,
            "1" => false,
            _ => return Err(TemplateError::Unsupported),
        };
        let nillable = match node.attribute("nillable").unwrap_or("false") {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err(TemplateError::Malformed),
        };
        let mut field = Field {
            xml_name: xml_name.into(),
            kind: Kind::String,
            optional,
            nillable,
            max_length: None,
            total_digits: None,
            fraction_digits: None,
        };
        let children = children(node)?;
        let base = if let Some(kind) = node.qualified("type") {
            if !children.is_empty() {
                return Err(TemplateError::Unsupported);
            }
            kind
        } else {
            if children.len() != 1 || !children[0].is(XSD, "simpleType") {
                return Err(TemplateError::Unsupported);
            }
            children[0].only_attributes(&[])?;
            let restriction = single(children[0], "restriction")?;
            restriction.only_attributes(&["base"])?;
            for facet in self::children(restriction)? {
                facet.only_attributes(&["value"])?;
                if facet.name.namespace != XSD || !self::children(facet)?.is_empty() {
                    return Err(TemplateError::Unsupported);
                }
                let value = facet.attribute("value").ok_or(TemplateError::Malformed)?;
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(TemplateError::Malformed);
                }
                let value = value.parse().map_err(|_| TemplateError::Unsupported)?;
                let slot = match facet.name.local.as_str() {
                    "maxLength" => &mut field.max_length,
                    "totalDigits" => &mut field.total_digits,
                    "fractionDigits" => &mut field.fraction_digits,
                    _ => return Err(TemplateError::Unsupported),
                };
                if slot.replace(value).is_some() {
                    return Err(TemplateError::Malformed);
                }
            }
            restriction
                .qualified("base")
                .ok_or(TemplateError::Malformed)?
        };
        if base.namespace != XSD {
            return Err(TemplateError::Unsupported);
        }
        field.kind = Kind::from_name(&base.local)?;
        if field.max_length.is_some()
            && !matches!(field.kind, Kind::String | Kind::Base64 | Kind::Hex)
            || (field.total_digits.is_some() || field.fraction_digits.is_some())
                && !matches!(field.kind, Kind::Decimal)
            || field.total_digits == Some(0)
            || field
                .fraction_digits
                .zip(field.total_digits)
                .is_some_and(|(fraction, total)| fraction > total)
        {
            return Err(TemplateError::Malformed);
        }
        let declared = if matches!(field.kind, Kind::String) {
            field
                .max_length
                .filter(|length| *length > 0)
                .map(TypeDescriptor::text)
                .transpose()
                .map_err(|_| TemplateError::Malformed)?
                .unwrap_or(TypeDescriptor::new(ScalarType::Text))
        } else {
            TypeDescriptor::new(field.kind.scalar())
        };
        budget
            .charge(
                Resource::Bytes,
                (size_of::<Field>() + size_of::<Column>() + name.len() + xml_name.len()) as u64,
            )
            .map_err(InspectError::from)?;
        columns
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        columns.push(
            Column::with_type(name, declared, optional || nillable)
                .map_err(|_| TemplateError::Malformed)?,
        );
        fields
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        fields.push(field);
    }
    validate_columns(&columns, NameComparison::AsciiInsensitive)
        .map_err(|_| TemplateError::Malformed)?;
    Ok(Schema {
        root: root
            .attribute("name")
            .ok_or(TemplateError::Malformed)?
            .into(),
        row: row_name.into(),
        columns,
        fields,
        generated: root_children.len() == 2,
    })
}

fn xml_name(value: &str) -> Result<&str, TemplateError> {
    if !value.chars().enumerate().all(|(index, c)| {
        c == '_'
            || c.is_alphabetic()
            || (index > 0 && (c.is_alphanumeric() || matches!(c, '-' | '.')))
    }) || value.is_empty()
    {
        return Err(TemplateError::Unsupported);
    }
    Ok(value)
}
fn children(node: &Node) -> Result<Vec<&Node>, TemplateError> {
    node.whitespace()?;
    Ok(node
        .children
        .iter()
        .filter(|node| node.name.namespace != OD && !node.is(XSD, "annotation"))
        .collect())
}
fn single<'a>(node: &'a Node, kind: &str) -> Result<&'a Node, TemplateError> {
    let children = children(node)?;
    if children.len() != 1 || !children[0].is(XSD, kind) {
        return Err(TemplateError::Unsupported);
    }
    Ok(children[0])
}
