//! Bounded inert XML trees for template metadata and inline table schemas.
use super::{TemplateError, check, encoding};
use quick_xml::{
    NsReader, XmlVersion,
    events::Event,
    name::{QName, ResolveResult},
};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const XSD: &str = "http://www.w3.org/2001/XMLSchema";
pub(super) const OD: &str = "urn:schemas-microsoft-com:officedata";
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 100_000;
const MAX_ATTRIBUTES: usize = 64;
const NODE_WORK_BYTES: u64 = 512;
const PARSER_WORK_FACTOR: u64 = 8;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Name {
    pub namespace: String,
    pub local: String,
}
pub(super) struct Attribute {
    pub name: Name,
    pub value: String,
    pub qualified: Option<Name>,
}
pub(super) struct Node {
    pub name: Name,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Node>,
    pub text: String,
}
impl Node {
    pub fn is(&self, namespace: &str, local: &str) -> bool {
        self.name.namespace == namespace && self.name.local == local
    }
    pub fn attribute(&self, local: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.name.namespace.is_empty() && a.name.local == local)
            .map(|a| a.value.as_str())
    }
    pub fn qualified(&self, local: &str) -> Option<&Name> {
        self.attributes
            .iter()
            .find(|a| a.name.namespace.is_empty() && a.name.local == local)
            .and_then(|a| a.qualified.as_ref())
    }
    pub fn scalar(&self) -> Result<&str, TemplateError> {
        if !self.children.is_empty() || !self.attributes.is_empty() {
            return Err(TemplateError::Unsupported);
        }
        Ok(&self.text)
    }
    pub fn whitespace(&self) -> Result<(), TemplateError> {
        if !self.text.bytes().all(|byte| byte.is_ascii_whitespace()) {
            return Err(TemplateError::Malformed);
        }
        Ok(())
    }
    pub fn only_attributes(&self, allowed: &[&str]) -> Result<(), TemplateError> {
        if self.attributes.iter().any(|a| {
            a.name.namespace != OD
                && (!a.name.namespace.is_empty() || !allowed.contains(&a.name.local.as_str()))
        }) {
            return Err(TemplateError::Unsupported);
        }
        Ok(())
    }
}

fn expanded(
    ns: ResolveResult<'_>,
    local: &str,
    budget: &mut ResourceBudget,
) -> Result<Name, TemplateError> {
    let namespace = match ns {
        ResolveResult::Bound(ref ns) => ns.as_ref(),
        ResolveResult::Unbound => "",
        ResolveResult::Unknown(_) => return Err(TemplateError::Malformed),
    };
    budget
        .charge(Resource::Bytes, (namespace.len() + local.len()) as u64)
        .map_err(InspectError::from)?;
    Ok(Name {
        namespace: namespace.to_owned(),
        local: local.to_owned(),
    })
}

pub(super) fn parse(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Node, TemplateError> {
    let decoded = encoding::decode(bytes, budget, cancelled)?;
    parse_decoded(&decoded.text, decoded.encoding, budget, cancelled)
}

pub(super) fn parse_decoded(
    text: &str,
    source_encoding: encoding::Encoding,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Node, TemplateError> {
    if text.len() > encoding::MAX_XML_BYTES {
        return Err(TemplateError::Unsupported);
    }
    let bytes = text.as_bytes();
    check(budget, cancelled)?;
    budget
        .charge(Resource::Bytes, bytes.len() as u64 * PARSER_WORK_FACTOR)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    if text.chars().any(|c| {
        !matches!(c, '\t' | '\r' | '\n') && (c < ' ' || matches!(c, '\u{fffe}' | '\u{ffff}'))
    }) {
        return Err(TemplateError::Malformed);
    }
    let mut reader = NsReader::from_reader(bytes);
    reader.config_mut().check_comments = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut declaration = false;
    let mut nodes = 0;
    loop {
        check(budget, cancelled)?;
        let event = reader.read_event().map_err(|_| TemplateError::Malformed)?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if root.is_some() || stack.len() >= MAX_DEPTH || nodes >= MAX_NODES {
                    return Err(TemplateError::Unsupported);
                }
                nodes += 1;
                budget
                    .charge(Resource::Bytes, NODE_WORK_BYTES)
                    .map_err(InspectError::from)?;
                let (namespace, local) = reader.resolver().resolve_element(element.name());
                let name = expanded(namespace, local.as_ref(), budget)?;
                let mut attributes: Vec<Attribute> = Vec::new();
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
                    let (ns, local) = reader.resolver().resolve_attribute(attribute.key);
                    let attr_name = expanded(ns, local.as_ref(), budget)?;
                    if attributes.iter().any(|a| a.name == attr_name) {
                        return Err(TemplateError::Malformed);
                    }
                    let value = attribute
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| TemplateError::Malformed)?
                        .into_owned();
                    let qualified = if name.namespace == XSD
                        && attr_name.namespace.is_empty()
                        && matches!(attr_name.local.as_str(), "type" | "base" | "ref")
                    {
                        let (ns, local) = reader.resolver().resolve_element(QName(
                            value.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n')),
                        ));
                        Some(expanded(ns, local.as_ref(), budget)?)
                    } else {
                        None
                    };
                    attributes
                        .try_reserve(1)
                        .map_err(|_| InspectError::Allocation)?;
                    attributes.push(Attribute {
                        name: attr_name,
                        value,
                        qualified,
                    });
                }
                let node = Node {
                    name,
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                };
                if matches!(event, Event::Start(_)) {
                    stack.try_reserve(1).map_err(|_| InspectError::Allocation)?;
                    stack.push(node);
                } else {
                    attach(node, &mut stack, &mut root)?;
                }
            }
            Event::End(_) => {
                let node = stack.pop().ok_or(TemplateError::Malformed)?;
                attach(node, &mut stack, &mut root)?;
            }
            Event::Text(text) => append(
                text.xml_content(XmlVersion::Implicit1_0).as_ref(),
                &mut stack,
            )?,
            Event::CData(text) => {
                if stack.is_empty() {
                    return Err(TemplateError::Malformed);
                }
                append(
                    text.xml_content(XmlVersion::Implicit1_0).as_ref(),
                    &mut stack,
                )?;
            }
            Event::GeneralRef(reference) => {
                if stack.is_empty() {
                    return Err(TemplateError::Malformed);
                }
                if let Some(character) = reference
                    .resolve_char_ref()
                    .map_err(|_| TemplateError::Malformed)?
                {
                    let mut buffer = [0; 4];
                    append(character.encode_utf8(&mut buffer), &mut stack)?;
                } else {
                    append(
                        quick_xml::escape::resolve_xml_entity(reference.as_ref())
                            .ok_or(TemplateError::Malformed)?,
                        &mut stack,
                    )?;
                }
            }
            Event::Comment(_) => {}
            Event::Decl(value) if !declaration && root.is_none() && stack.is_empty() => {
                declaration = true;
                encoding::declaration(&value, source_encoding)?;
            }
            Event::Eof if stack.is_empty() => return root.ok_or(TemplateError::Malformed),
            _ => return Err(TemplateError::Malformed),
        }
    }
}

fn attach(node: Node, stack: &mut [Node], root: &mut Option<Node>) -> Result<(), TemplateError> {
    if let Some(parent) = stack.last_mut() {
        parent
            .children
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(TemplateError::Malformed);
    }
    Ok(())
}
fn append(text: &str, stack: &mut [Node]) -> Result<(), TemplateError> {
    if text.chars().any(|c| {
        !matches!(c, '\t' | '\r' | '\n') && (c < ' ' || matches!(c, '\u{fffe}' | '\u{ffff}'))
    }) {
        return Err(TemplateError::Malformed);
    }
    if let Some(node) = stack.last_mut() {
        node.text
            .try_reserve(text.len())
            .map_err(|_| InspectError::Allocation)?;
        node.text.push_str(text);
    } else if !text.bytes().all(|byte| byte.is_ascii_whitespace()) {
        return Err(TemplateError::Malformed);
    }
    Ok(())
}
