//! Flat OPC control XML with explicit namespace and allocation/work ceilings.
use super::{TemplateError, check, encoding};
use quick_xml::{NsReader, XmlVersion, events::Event, name::ResolveResult};
use std::collections::BTreeMap;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_ATTRIBUTES: usize = 32;
const MAX_RECORDS: usize = 4096;
const ATTRIBUTE_WORK_BYTES: usize = 128;

pub(super) struct Record {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
}

pub(super) fn records(
    bytes: &[u8],
    namespace: &str,
    root: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Record>, TemplateError> {
    let decoded = encoding::decode(bytes, budget, cancelled)?;
    let bytes = decoded.text.as_bytes();
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let mut reader = NsReader::from_reader(bytes);
    let mut seen_root = false;
    let mut seen_declaration = false;
    let mut finished = false;
    let mut depth = 0;
    let mut records = Vec::new();
    loop {
        check(budget, cancelled)?;
        let event = reader.read_event().map_err(|_| TemplateError::Malformed)?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if finished || depth > 1 {
                    return Err(TemplateError::Malformed);
                }
                let (ns, local) = reader.resolver().resolve_element(element.name());
                if !matches!(ns, ResolveResult::Bound(value) if value.as_ref() == namespace) {
                    return Err(TemplateError::Unsupported);
                }
                let mut attributes = BTreeMap::new();
                for (index, attribute) in element.attributes().enumerate() {
                    check(budget, cancelled)?;
                    if index >= MAX_ATTRIBUTES {
                        return Err(TemplateError::Unsupported);
                    }
                    let attribute = attribute.map_err(|_| TemplateError::Malformed)?;
                    if attribute.key.as_ref() == "xmlns"
                        || attribute.key.as_ref().starts_with("xmlns:")
                    {
                        continue;
                    }
                    let (ns, key) = reader.resolver().resolve_attribute(attribute.key);
                    if !matches!(ns, ResolveResult::Unbound) {
                        return Err(TemplateError::Unsupported);
                    }
                    let key = key.as_ref();
                    let value = attribute
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| TemplateError::Malformed)?;
                    budget
                        .charge(
                            Resource::Bytes,
                            (3 * (key.len() + value.len()) + ATTRIBUTE_WORK_BYTES) as u64,
                        )
                        .map_err(InspectError::from)?;
                    if attributes
                        .insert(key.to_owned(), value.into_owned())
                        .is_some()
                    {
                        return Err(TemplateError::Malformed);
                    }
                }
                if depth == 0 {
                    if seen_root || local.as_ref() != root || !attributes.is_empty() {
                        return Err(TemplateError::Malformed);
                    }
                    seen_root = true;
                } else {
                    if records.len() >= MAX_RECORDS {
                        return Err(TemplateError::Unsupported);
                    }
                    budget
                        .charge(
                            Resource::Bytes,
                            (local.as_ref().len() + size_of::<Record>()) as u64,
                        )
                        .map_err(InspectError::from)?;
                    records
                        .try_reserve(1)
                        .map_err(|_| InspectError::Allocation)?;
                    records.push(Record {
                        name: local.as_ref().into(),
                        attributes,
                    });
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                } else if depth == 0 {
                    finished = true;
                }
            }
            Event::End(_) => {
                if depth == 0 {
                    return Err(TemplateError::Malformed);
                }
                depth -= 1;
                if depth == 0 {
                    finished = true;
                }
            }
            Event::Text(text) if text.as_ref().bytes().all(|byte| byte.is_ascii_whitespace()) => {}
            Event::Comment(_) => {}
            Event::Decl(declaration) if !seen_root && !seen_declaration => {
                seen_declaration = true;
                encoding::declaration(&declaration, decoded.encoding)?;
            }
            Event::Eof if seen_root && finished => return Ok(records),
            _ => return Err(TemplateError::Malformed),
        }
    }
}
