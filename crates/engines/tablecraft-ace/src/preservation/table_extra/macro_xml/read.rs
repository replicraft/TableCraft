use super::{DataMacroEnvelope, DataMacroXml, DataMacroXmlNamespace, names};
use crate::{InspectError, header::check, limits::MAX_LONG_VALUE_BYTES};
use quick_xml::{
    NsReader, XmlVersion,
    events::{BytesRef, BytesStart, Event},
    name::{NamespaceError, ResolveResult},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) mod text;

const MAX_DEPTH: usize = 128;
const MAX_NAMESPACE_BINDINGS: usize = 128;
const MAX_DEFINITIONS: usize = 1024;
const MAX_ATTRIBUTES: usize = 64;
const PARSER_ALLOCATION_FACTOR: u64 = 8;

#[derive(Clone, Copy)]
enum Node {
    Wrapper,
    Definition,
    Body,
}

pub(super) fn document(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DataMacroXml, InspectError> {
    inspect(text, false, budget, cancelled)
}

pub(super) fn program_envelopes(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DataMacroXml, InspectError> {
    inspect(text, true, budget, cancelled)
}

fn inspect(
    text: &str,
    program: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DataMacroXml, InspectError> {
    let _span = tracing::debug_span!("native.table.extra.macro_xml.inspect").entered();
    check(budget, cancelled)?;
    if text.len() > MAX_LONG_VALUE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    // quick-xml owns bounded namespace, attribute and unescape buffers. Charge
    // conservatively before invoking the parser; retained outputs are separate.
    budget.charge_allocation(text.len() as u64, PARSER_ALLOCATION_FACTOR)?;
    for scalar in text.chars() {
        check(budget, cancelled)?;
        if !legal_character(scalar) {
            return Err(InspectError::Malformed);
        }
    }

    let mut reader = NsReader::from_str(text.strip_prefix('\u{feff}').unwrap_or(text));
    reader.config_mut().check_comments = true;
    reader
        .resolver_mut()
        .set_max_namespace_bindings(MAX_NAMESPACE_BINDINGS);
    let mut stack = [Node::Body; MAX_DEPTH];
    let mut depth = 0;
    let mut namespace = None;
    let mut root_seen = false;
    let mut declaration_seen = false;
    let mut definitions = Vec::new();

    loop {
        check(budget, cancelled)?;
        let start = reader.buffer_position();
        let event = reader.read_event().map_err(|error| match error {
            quick_xml::Error::Namespace(NamespaceError::TooManyBindings(_)) => {
                InspectError::StructureLimit
            }
            _ => InspectError::Malformed,
        })?;
        budget.charge(Resource::Steps, reader.buffer_position() - start)?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if depth == MAX_DEPTH {
                    return Err(InspectError::StructureLimit);
                }
                names::qualified(element.name().as_ref(), budget, cancelled)?;
                resolution_work(&reader, element.name().as_ref(), budget)?;
                let (resolved, local) = reader.resolver().resolve_element(element.name());
                let parent = depth.checked_sub(1).map(|index| stack[index]);
                let in_body = matches!(parent, Some(Node::Definition | Node::Body));
                let current_namespace = match resolved {
                    ResolveResult::Bound(value)
                        if value.as_ref() == DataMacroXmlNamespace::April2009.uri() =>
                    {
                        Some(DataMacroXmlNamespace::April2009)
                    }
                    ResolveResult::Bound(value)
                        if value.as_ref() == DataMacroXmlNamespace::November2009.uri() =>
                    {
                        Some(DataMacroXmlNamespace::November2009)
                    }
                    ResolveResult::Unknown(_) => return Err(InspectError::Unsupported),
                    _ => None,
                };
                if !(program && in_body) {
                    let current_namespace = current_namespace.ok_or(InspectError::Unsupported)?;
                    if namespace.is_some_and(|expected| expected != current_namespace) {
                        return Err(InspectError::Unsupported);
                    }
                    namespace = Some(current_namespace);
                }
                let node = match (parent, local.as_ref()) {
                    (None, "DataMacros") if !root_seen => Node::Wrapper,
                    (None, "DataMacro") if !root_seen => Node::Definition,
                    (Some(Node::Wrapper), "DataMacro") => Node::Definition,
                    (Some(Node::Definition | Node::Body), name)
                        if program || (name != "DataMacro" && name != "DataMacros") =>
                    {
                        Node::Body
                    }
                    _ => return Err(InspectError::Unsupported),
                };
                if parent.is_none() {
                    root_seen = true;
                }
                let envelope = attributes(element, &reader, node, budget, cancelled)?;
                if let Some(envelope) = envelope {
                    if definitions.len() == MAX_DEFINITIONS {
                        return Err(InspectError::StructureLimit);
                    }
                    budget.charge_allocation(1, size_of::<DataMacroEnvelope>() as u64)?;
                    definitions
                        .try_reserve(1)
                        .map_err(|_| InspectError::Allocation)?;
                    definitions.push(envelope);
                }
                if matches!(event, Event::Start(_)) {
                    stack[depth] = node;
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(InspectError::Malformed)?;
            }
            Event::Text(content) => {
                if content.as_ref().contains("]]>") {
                    return Err(InspectError::Malformed);
                }
                if (depth == 0 || matches!(stack[depth - 1], Node::Wrapper))
                    && !content
                        .as_ref()
                        .bytes()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    return Err(InspectError::Malformed);
                }
            }
            Event::CData(_) if depth > 0 && !matches!(stack[depth - 1], Node::Wrapper) => {}
            Event::GeneralRef(reference)
                if depth > 0 && !matches!(stack[depth - 1], Node::Wrapper) =>
            {
                reference_character(&reference)?;
            }
            Event::Comment(_) => {}
            Event::Decl(declaration) if start == 0 && !declaration_seen && !root_seen => {
                declaration_seen = true;
                declaration_shape(declaration.as_ref(), budget, cancelled)?;
                if declaration.version().map_err(|_| InspectError::Malformed)? != "1.0" {
                    return Err(InspectError::Unsupported);
                }
                if let Some(encoding) = declaration.encoding() {
                    let encoding = encoding.map_err(|_| InspectError::Malformed)?;
                    // The native property reader already decoded strict UTF-16.
                    // XML declarations do not select a second byte decoder.
                    if !["UTF-8", "UTF-16", "UTF-16LE"]
                        .iter()
                        .any(|known| encoding.eq_ignore_ascii_case(known))
                    {
                        return Err(InspectError::Unsupported);
                    }
                }
                if let Some(standalone) = declaration.standalone()
                    && !matches!(
                        standalone.map_err(|_| InspectError::Malformed)?.as_ref(),
                        "yes" | "no"
                    )
                {
                    return Err(InspectError::Malformed);
                }
            }
            Event::Eof => break,
            // DTDs, custom entity declarations, processing instructions and
            // misplaced declarations never initiate external resolution.
            _ => return Err(InspectError::Unsupported),
        }
    }
    if depth != 0 || !root_seen || definitions.is_empty() {
        return Err(InspectError::Malformed);
    }
    check(budget, cancelled)?;
    Ok(DataMacroXml {
        namespace: namespace.ok_or(InspectError::Malformed)?,
        definitions,
    })
}

fn attributes(
    element: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
    node: Node,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<DataMacroEnvelope>, InspectError> {
    let mut envelope = DataMacroEnvelope {
        event: None,
        name: None,
    };
    for (index, attribute) in element.attributes().enumerate() {
        check(budget, cancelled)?;
        if index == MAX_ATTRIBUTES {
            return Err(InspectError::StructureLimit);
        }
        let attribute = attribute.map_err(|_| InspectError::Malformed)?;
        names::qualified(attribute.key.as_ref(), budget, cancelled)?;
        if attribute.value.contains('<') {
            return Err(InspectError::Malformed);
        }
        let value = attribute
            .normalized_value(XmlVersion::Explicit1_0)
            .map_err(|_| InspectError::Malformed)?;
        for scalar in value.chars() {
            check(budget, cancelled)?;
            if !legal_character(scalar) {
                return Err(InspectError::Malformed);
            }
        }
        if attribute.key.as_ref() == "xmlns" || attribute.key.as_ref().starts_with("xmlns:") {
            // The resolver's namespace keys use lexical URI values. Escaped or
            // whitespace-normalized URI aliases need a different resolver
            // profile; do not accidentally miss expanded-attribute duplicates.
            if attribute.value.as_ref() != value.as_ref() {
                return Err(InspectError::Unsupported);
            }
            continue;
        }
        resolution_work(reader, attribute.key.as_ref(), budget)?;
        let (resolved, local) = reader.resolver().resolve_attribute(attribute.key);
        if matches!(resolved, ResolveResult::Unknown(_)) {
            return Err(InspectError::Unsupported);
        }
        for earlier in element.attributes().take(index) {
            check(budget, cancelled)?;
            let earlier = earlier.map_err(|_| InspectError::Malformed)?;
            resolution_work(reader, earlier.key.as_ref(), budget)?;
            let (earlier_namespace, earlier_local) =
                reader.resolver().resolve_attribute(earlier.key);
            budget.charge(
                Resource::Steps,
                (attribute.key.as_ref().len().max(earlier.key.as_ref().len())
                    + namespace_length(&resolved).max(namespace_length(&earlier_namespace)))
                    as u64,
            )?;
            if earlier_namespace == resolved && earlier_local == local {
                return Err(InspectError::Malformed);
            }
        }
        if matches!(node, Node::Body) {
            continue;
        }
        if !matches!(resolved, ResolveResult::Unbound) || matches!(node, Node::Wrapper) {
            return Err(InspectError::Unsupported);
        }
        let field = match local.as_ref() {
            "Event" => &mut envelope.event,
            "Name" => &mut envelope.name,
            _ => return Err(InspectError::Unsupported),
        };
        if field.is_some() || value.is_empty() {
            return Err(InspectError::Malformed);
        }
        budget.charge_allocation(value.len() as u64, size_of::<u8>() as u64)?;
        let mut retained = String::new();
        retained
            .try_reserve_exact(value.len())
            .map_err(|_| InspectError::Allocation)?;
        retained.push_str(&value);
        *field = Some(retained);
    }
    if matches!(node, Node::Definition) {
        if envelope.name.is_some() == envelope.event.is_some() {
            return Err(InspectError::Malformed);
        }
        Ok(Some(envelope))
    } else {
        Ok(None)
    }
}

fn legal_character(scalar: char) -> bool {
    matches!(scalar, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn reference_character(reference: &BytesRef<'_>) -> Result<char, InspectError> {
    if let Some(scalar) = reference
        .resolve_char_ref()
        .map_err(|_| InspectError::Malformed)?
    {
        return legal_character(scalar)
            .then_some(scalar)
            .ok_or(InspectError::Malformed);
    }

    match reference.as_ref() {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "quot" => Ok('"'),
        "apos" => Ok('\''),
        _ => Err(InspectError::Unsupported),
    }
}

fn resolution_work(
    reader: &NsReader<&[u8]>,
    name: &str,
    budget: &mut ResourceBudget,
) -> Result<(), InspectError> {
    let bindings = reader
        .resolver()
        .bindings()
        .size_hint()
        .1
        .ok_or(InspectError::StructureLimit)?;
    let cost = bindings
        .checked_add(1)
        .and_then(|count| count.checked_mul(name.len()))
        .ok_or(InspectError::StructureLimit)?;
    budget.charge(Resource::Steps, cost as u64)?;
    Ok(())
}

fn namespace_length(namespace: &ResolveResult<'_>) -> usize {
    match namespace {
        ResolveResult::Bound(value) => value.as_ref().len(),
        _ => 0,
    }
}

fn declaration_shape(
    raw: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let declaration = BytesStart::from_content(raw, "xml".len());
    let fields = ["version", "encoding", "standalone"];
    let mut previous = None;
    for attribute in declaration.attributes() {
        check(budget, cancelled)?;
        let attribute = attribute.map_err(|_| InspectError::Malformed)?;
        let position = fields
            .iter()
            .position(|field| *field == attribute.key.as_ref())
            .ok_or(InspectError::Unsupported)?;
        if previous.is_some_and(|earlier| position <= earlier)
            || (previous.is_none() && position != 0)
        {
            return Err(InspectError::Malformed);
        }
        previous = Some(position);
    }
    if previous.is_none() {
        return Err(InspectError::Malformed);
    }
    Ok(())
}
