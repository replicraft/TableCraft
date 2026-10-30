//! Bounded source-position arena after the common lexical validator completes.
use super::XmlByteSpan;
use crate::{InspectError, header::check};
use quick_xml::{
    NsReader, XmlVersion,
    events::{BytesStart, Event},
    name::ResolveResult,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_XML_ELEMENTS: usize = 16_384;
const MAX_DEPTH: usize = 128;
const MAX_NAMESPACE_BINDINGS: usize = 128;
const PARSER_ALLOCATION_FACTOR: u64 = 8;
const INITIAL_CAPACITY: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Wrapper,
    Definition,
    Statements,
    ConditionalBlock,
    If,
    Condition,
    Action,
    Argument,
    Other,
    Foreign,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Name {
    RaiseError,
    SetField,
    Number,
    Description,
    Other,
}

#[derive(Default)]
pub(super) struct Attributes {
    pub name: Option<Name>,
    pub foreign: bool,
    pub extra: bool,
}

impl Attributes {
    pub fn plain(&self) -> bool {
        self.name.is_none() && !self.foreign && !self.extra
    }
}

pub(super) struct Element {
    pub kind: Kind,
    pub attributes: Attributes,
    pub span: XmlByteSpan,
    pub content: XmlByteSpan,
    pub first: Option<usize>,
    pub next: Option<usize>,
    pub nonwhite_text: bool,
    last: Option<usize>,
}

pub(super) struct Tree {
    pub elements: Vec<Element>,
    pub root: usize,
}

impl Tree {
    pub fn children(&self, element: usize) -> Children<'_> {
        Children {
            tree: self,
            next: self.elements[element].first,
        }
    }
}

pub(super) struct Children<'a> {
    tree: &'a Tree,
    next: Option<usize>,
}

impl Iterator for Children<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let current = self.next?;
        self.next = self.tree.elements[current].next;
        Some(current)
    }
}

pub(super) fn scan(
    text: &str,
    namespace: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Tree, InspectError> {
    check(budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, PARSER_ALLOCATION_FACTOR)?;
    let input = text.strip_prefix('\u{feff}').unwrap_or(text);
    let base = text.len() - input.len();
    let mut reader = NsReader::from_str(input);
    reader
        .resolver_mut()
        .set_max_namespace_bindings(MAX_NAMESPACE_BINDINGS);
    let mut elements: Vec<Element> = Vec::new();
    let mut stack = [0_usize; MAX_DEPTH];
    let mut depth = 0;
    let mut root = None;

    loop {
        check(budget, cancelled)?;
        let before = reader.buffer_position();
        let event = reader.read_event().map_err(|_| InspectError::Malformed)?;
        let after = reader.buffer_position();
        budget.charge(Resource::Steps, after - before)?;
        let start = base
            .checked_add(usize::try_from(before).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(InspectError::StructureLimit)?;
        let end = base
            .checked_add(usize::try_from(after).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(InspectError::StructureLimit)?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if depth == MAX_DEPTH {
                    return Err(InspectError::StructureLimit);
                }
                namespace_work(&reader, element.name().as_ref().len(), budget)?;
                let (resolved, local) = reader.resolver().resolve_element(element.name());
                let kind = match resolved {
                    ResolveResult::Bound(uri) if uri.as_ref() == namespace => {
                        match local.as_ref() {
                            "DataMacros" => Kind::Wrapper,
                            "DataMacro" => Kind::Definition,
                            "Statements" => Kind::Statements,
                            "ConditionalBlock" => Kind::ConditionalBlock,
                            "If" => Kind::If,
                            "Condition" => Kind::Condition,
                            "Action" => Kind::Action,
                            "Argument" => Kind::Argument,
                            _ => Kind::Other,
                        }
                    }
                    ResolveResult::Unknown(_) => return Err(InspectError::Unsupported),
                    _ => Kind::Foreign,
                };
                let attributes = attributes(element, &reader, budget, cancelled)?;
                reserve(&mut elements, MAX_XML_ELEMENTS, budget)?;
                let index = elements.len();
                elements.push(Element {
                    kind,
                    attributes,
                    span: XmlByteSpan { start, end },
                    content: XmlByteSpan { start: end, end },
                    first: None,
                    last: None,
                    next: None,
                    nonwhite_text: false,
                });
                if depth == 0 {
                    if root.replace(index).is_some() {
                        return Err(InspectError::Malformed);
                    }
                } else {
                    let parent = stack[depth - 1];
                    if let Some(previous) = elements[parent].last {
                        elements[previous].next = Some(index);
                    } else {
                        elements[parent].first = Some(index);
                    }
                    elements[parent].last = Some(index);
                }
                if matches!(event, Event::Start(_)) {
                    stack[depth] = index;
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(InspectError::Malformed)?;
                let current = &mut elements[stack[depth]];
                current.content.end = start;
                current.span.end = end;
            }
            Event::Text(content) => {
                if depth > 0
                    && !content
                        .as_ref()
                        .bytes()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    elements[stack[depth - 1]].nonwhite_text = true;
                }
            }
            Event::CData(_) | Event::GeneralRef(_) if depth > 0 => {
                elements[stack[depth - 1]].nonwhite_text = true;
            }
            Event::Eof => break,
            // The first pass has already validated all other lexical forms.
            _ => {}
        }
    }
    if depth != 0 {
        return Err(InspectError::Malformed);
    }
    check(budget, cancelled)?;
    Ok(Tree {
        elements,
        root: root.ok_or(InspectError::Malformed)?,
    })
}

fn attributes(
    element: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Attributes, InspectError> {
    let mut result = Attributes::default();
    for attribute in element.attributes() {
        check(budget, cancelled)?;
        let attribute = attribute.map_err(|_| InspectError::Malformed)?;
        if attribute.key.as_ref() == "xmlns" || attribute.key.as_ref().starts_with("xmlns:") {
            continue;
        }
        namespace_work(reader, attribute.key.as_ref().len(), budget)?;
        let (resolved, local) = reader.resolver().resolve_attribute(attribute.key);
        match resolved {
            ResolveResult::Unbound if local.as_ref() == "Name" => {
                let value = attribute
                    .normalized_value(XmlVersion::Explicit1_0)
                    .map_err(|_| InspectError::Malformed)?;
                budget.charge(Resource::Steps, value.len() as u64)?;
                result.name = Some(match value.as_ref() {
                    "RaiseError" => Name::RaiseError,
                    "SetField" => Name::SetField,
                    "Number" => Name::Number,
                    "Description" => Name::Description,
                    _ => Name::Other,
                });
            }
            ResolveResult::Unbound => result.extra = true,
            ResolveResult::Unknown(_) => return Err(InspectError::Unsupported),
            _ => result.foreign = true,
        }
    }
    Ok(result)
}

fn namespace_work(
    reader: &NsReader<&[u8]>,
    bytes: usize,
    budget: &mut ResourceBudget,
) -> Result<(), InspectError> {
    let bindings = reader
        .resolver()
        .bindings()
        .size_hint()
        .1
        .ok_or(InspectError::StructureLimit)?;
    let work = bindings
        .checked_add(1)
        .and_then(|count| count.checked_mul(bytes))
        .ok_or(InspectError::StructureLimit)?;
    budget.charge(Resource::Steps, work as u64)?;
    Ok(())
}

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    maximum: usize,
    budget: &mut ResourceBudget,
) -> Result<(), InspectError> {
    if values.len() == maximum {
        return Err(InspectError::StructureLimit);
    }
    if values.len() == values.capacity() {
        let extra = values
            .capacity()
            .max(INITIAL_CAPACITY)
            .min(maximum - values.len());
        budget.charge_allocation(extra as u64, size_of::<T>() as u64)?;
        budget.charge(Resource::Steps, values.len() as u64)?;
        values
            .try_reserve_exact(extra)
            .map_err(|_| InspectError::Allocation)?;
    }
    Ok(())
}
