//! Inline XSD table-data decoding without instantiating an Access application.
mod list;
mod names;
pub(super) mod schema;
mod values;

use super::{TemplateError, TemplateObject, TemplatePackage, check, encoding, tree};
use jiff::{Timestamp, civil::DateTime};
use quick_xml::{NsReader, events::Event, name::ResolveResult};
use tablecraft_ace::InspectError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::ObjectKind;
use tablecraft_schema::{Column, validate_row};

const MAX_COLUMNS: usize = 256;
const MAX_ROWS: usize = 100_000;
const MAX_SCHEMA_DEPTH: usize = 64;
const ROOT_END: &[u8] = b"</root>";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// An explicitly decoded immutable table-data snapshot. Types come from inline
/// XSD, not guessed from values or Office application properties. Stored native
/// field types and application/list-schema agreement remain separate contracts.
pub struct TemplateTable {
    name: String,
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
}
impl TemplateTable {
    /// Access object name, validated against the inline row element name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Stable available scalar columns in XSD sequence order.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Complete validated rows; no application objects or expressions ran.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }
}

impl TemplatePackage {
    /// Discover one local table's inline XSD columns without reading data-instance
    /// events. Qualified List Definition declarations are checked as inert
    /// metadata; no record values, defaults or application code are evaluated.
    pub fn table_columns(
        &self,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Column>, TemplateError> {
        let objects = self.objects(budget, cancelled)?;
        let object = objects
            .iter()
            .find(|object| {
                object.kind() == ObjectKind::Table && object.name().eq_ignore_ascii_case(name)
            })
            .ok_or(TemplateError::Unsupported)?;
        if object.is_linked() {
            return Err(TemplateError::Unsupported);
        }
        self.columns_for_object(object, budget, cancelled)
    }

    pub(super) fn columns_for_object(
        &self,
        object: &TemplateObject,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Column>, TemplateError> {
        if object.is_linked() {
            return Err(TemplateError::Unsupported);
        }
        let part = self
            .part(object.data_part().ok_or(TemplateError::Unsupported)?)
            .ok_or(TemplateError::Malformed)?;
        let schema = schema_prefix(part.bytes(), object.name(), budget, cancelled)?;
        list::validate(self, object, &schema, budget, cancelled)?;
        Ok(schema.columns)
    }

    /// Decode the stored data explicitly related to a named local table. A
    /// missing part is refused rather than represented as an invented empty table.
    /// Linked objects are never retrieved. All rows validate before publication.
    pub fn read_table(
        &self,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TemplateTable, TemplateError> {
        let _span = tracing::debug_span!("access.template.table.read").entered();
        let objects = self.objects(budget, cancelled)?;
        let mut matches = objects.iter().filter(|object| {
            object.kind() == ObjectKind::Table && object.name().eq_ignore_ascii_case(name)
        });
        let object = matches.next().ok_or(TemplateError::Unsupported)?;
        if matches.next().is_some() {
            return Err(TemplateError::Malformed);
        }
        self.table_data(object, budget, cancelled)
    }

    fn table_data(
        &self,
        object: &TemplateObject,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TemplateTable, TemplateError> {
        if object.is_linked() {
            return Err(TemplateError::Unsupported);
        }
        let part = self
            .part(object.data_part().ok_or(TemplateError::Unsupported)?)
            .ok_or(TemplateError::Malformed)?;
        let root = tree::parse(part.bytes(), budget, cancelled)?;
        root.whitespace()?;
        if !root.is("", "root") || !root.attributes.is_empty() || root.children.len() != 2 {
            return Err(TemplateError::Unsupported);
        }
        let schema = schema::read(&root.children[0], object.name(), budget, cancelled)?;
        list::validate(self, object, &schema, budget, cancelled)?;
        let instance = &root.children[1];
        instance.whitespace()?;
        instance.only_attributes(&["generated"])?;
        if instance
            .attributes
            .iter()
            .any(|attribute| !attribute.name.namespace.is_empty())
        {
            return Err(TemplateError::Unsupported);
        }
        if let Some(generated) = instance.attribute("generated")
            && (!schema.generated
                || (generated.parse::<DateTime>().is_err()
                    && generated.parse::<Timestamp>().is_err()))
        {
            return Err(TemplateError::Malformed);
        }
        if !instance.is("", &schema.root) {
            return Err(TemplateError::Malformed);
        }
        let mut rows = Vec::new();
        for node in &instance.children {
            check(budget, cancelled)?;
            if rows.len() >= MAX_ROWS {
                return Err(TemplateError::Unsupported);
            }
            budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            budget
                .charge(
                    Resource::Bytes,
                    (schema.fields.len() * size_of::<Value>() + size_of::<Vec<Value>>()) as u64,
                )
                .map_err(InspectError::from)?;
            if !node.is("", &schema.row) || !node.attributes.is_empty() {
                return Err(TemplateError::Malformed);
            }
            node.whitespace()?;
            let mut row = Vec::new();
            row.try_reserve_exact(schema.fields.len())
                .map_err(|_| InspectError::Allocation)?;
            let mut cells = node.children.iter().peekable();
            for field in &schema.fields {
                check(budget, cancelled)?;
                let value = if cells
                    .peek()
                    .is_some_and(|node| node.is("", &field.xml_name))
                {
                    let cell = cells.next().ok_or(TemplateError::Malformed)?;
                    values::read(field, cell)?
                } else if field.optional {
                    Value::Null
                } else {
                    return Err(TemplateError::Malformed);
                };
                row.push(value);
            }
            if cells.next().is_some() {
                return Err(TemplateError::Malformed);
            }
            validate_row(&schema.columns, &row).map_err(|_| TemplateError::Malformed)?;
            rows.try_reserve(1).map_err(|_| InspectError::Allocation)?;
            rows.push(row);
        }
        check(budget, cancelled)?;
        Ok(TemplateTable {
            name: object.name().into(),
            columns: schema.columns,
            rows,
        })
    }
}

fn schema_prefix(
    bytes: &[u8],
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<schema::Schema, TemplateError> {
    // Copy only the first complete inline schema with its root namespace context.
    // No data-instance event or record value is requested during schema discovery.
    let mut reader = NsReader::from_reader(encoding::PrefixReader::new(bytes, budget, cancelled));
    reader.config_mut().check_comments = true;
    let mut depth = 0usize;
    let mut schema_seen = false;
    let mut event_buffer = Vec::new();
    loop {
        reader.get_mut().check()?;
        let event = match reader.read_event_into(&mut event_buffer) {
            Ok(event) => event,
            Err(_) => return Err(reader.get_mut().take_error()),
        };
        match event {
            Event::Start(element) => {
                depth += 1;
                if depth == 1 && element.local_name().as_ref() != "root" {
                    return Err(TemplateError::Malformed);
                }
                if depth == 2 {
                    let (ns, local) = reader.resolver().resolve_element(element.name());
                    if schema_seen
                        || local.as_ref() != "schema"
                        || !matches!(ns, ResolveResult::Bound(ns) if ns.as_ref() == tree::XSD)
                    {
                        return Err(TemplateError::Malformed);
                    }
                    schema_seen = true;
                }
                if depth > MAX_SCHEMA_DEPTH {
                    return Err(TemplateError::Unsupported);
                }
            }
            Event::End(_) => {
                if depth == 0 {
                    return Err(TemplateError::Malformed);
                }
                depth -= 1;
                if schema_seen && depth == 1 {
                    break;
                }
            }
            Event::Empty(_) if depth < 2 => return Err(TemplateError::Unsupported),
            Event::Decl(value) => encoding::declaration(&value, reader.get_ref().encoding())?,
            Event::DocType(_) | Event::PI(_) | Event::Eof => return Err(TemplateError::Malformed),
            _ => {}
        }
        event_buffer.clear();
    }
    let end = reader.get_ref().position();
    let source_encoding = reader.get_ref().encoding();
    drop(reader);
    let decoded = encoding::decode(&bytes[..end], budget, cancelled)?;
    let text = decoded.text.as_bytes();
    budget
        .charge(Resource::Bytes, (text.len() + ROOT_END.len()) as u64)
        .map_err(InspectError::from)?;
    let mut prefix = Vec::new();
    prefix
        .try_reserve_exact(text.len() + ROOT_END.len())
        .map_err(|_| InspectError::Allocation)?;
    prefix.extend_from_slice(text);
    prefix.extend_from_slice(ROOT_END);
    let text = std::str::from_utf8(&prefix).map_err(|_| TemplateError::Malformed)?;
    let root = tree::parse_decoded(text, source_encoding, budget, cancelled)?;
    if !root.is("", "root") || !root.attributes.is_empty() || root.children.len() != 1 {
        return Err(TemplateError::Unsupported);
    }
    schema::read(&root.children[0], name, budget, cancelled)
}

#[cfg(test)]
mod tests;
