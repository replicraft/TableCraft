//! Closed, exact-name local simple restrictions; no external schema resolution.
use super::{Kind, children, single, xml_name};
use crate::ReadError;
use crate::template::{
    TemplateError, check,
    tree::{Name, Node, XSD},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_NAMED_TYPES: usize = 256;
const MAX_TYPE_DEPTH: usize = 64;

#[derive(Clone, Copy)]
pub(super) struct Domain {
    pub kind: Kind,
    pub max_length: Option<usize>,
    pub total_digits: Option<usize>,
    pub fraction_digits: Option<usize>,
}

impl Domain {
    fn builtin(name: &str) -> Result<Self, TemplateError> {
        Ok(Self {
            kind: Kind::from_name(name)?,
            max_length: None,
            total_digits: None,
            fraction_digits: None,
        })
    }

    fn validate(self) -> Result<Self, TemplateError> {
        if self.max_length.is_some()
            && !matches!(self.kind, Kind::String | Kind::Base64 | Kind::Hex)
            || (self.total_digits.is_some() || self.fraction_digits.is_some())
                && !matches!(self.kind, Kind::Decimal)
            || self.total_digits == Some(0)
            || self
                .fraction_digits
                .zip(self.total_digits)
                .is_some_and(|(fraction, total)| fraction > total)
        {
            return Err(TemplateError::Malformed);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Default)]
struct Facets {
    max_length: Option<usize>,
    total_digits: Option<usize>,
    fraction_digits: Option<usize>,
}

impl Facets {
    fn apply(self, base: Domain) -> Result<Domain, TemplateError> {
        fn narrowed(
            base: Option<usize>,
            derived: Option<usize>,
        ) -> Result<Option<usize>, TemplateError> {
            if derived
                .zip(base)
                .is_some_and(|(derived, base)| derived > base)
            {
                return Err(TemplateError::Malformed);
            }
            Ok(derived.or(base))
        }

        Domain {
            kind: base.kind,
            max_length: narrowed(base.max_length, self.max_length)?,
            total_digits: narrowed(base.total_digits, self.total_digits)?,
            fraction_digits: narrowed(base.fraction_digits, self.fraction_digits)?,
        }
        .validate()
    }
}

#[derive(Clone, Copy)]
enum State {
    Unseen,
    Visiting,
    Resolved { domain: Domain, depth: usize },
}

struct Definition<'a> {
    name: &'a str,
    base: &'a Name,
    facets: Facets,
    state: State,
}

pub(super) struct LocalTypes<'a> {
    definitions: Vec<Definition<'a>>,
}

impl<'a> LocalTypes<'a> {
    pub fn new(
        declarations: &[&'a Node],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TemplateError> {
        let mut types = Self {
            definitions: Vec::new(),
        };
        for declaration in declarations {
            check(budget, cancelled)?;
            if declaration.is(XSD, "element") {
                continue;
            }
            if !declaration.is(XSD, "simpleType") || types.definitions.len() >= MAX_NAMED_TYPES {
                return Err(TemplateError::Unsupported);
            }
            declaration.only_attributes(&["name"])?;
            let name = xml_name(
                declaration
                    .attribute("name")
                    .ok_or(TemplateError::Malformed)?,
            )?;
            if types.find(name, budget, cancelled)?.is_some() {
                return Err(TemplateError::Malformed);
            }
            let (base, facets) = restriction(declaration, budget, cancelled)?;
            budget
                .charge(Resource::Bytes, size_of::<Definition>() as u64)
                .map_err(ReadError::from)?;
            types
                .definitions
                .try_reserve_exact(1)
                .map_err(|_| ReadError::Allocation)?;
            types.definitions.push(Definition {
                name,
                base,
                facets,
                state: State::Unseen,
            });
        }

        // Resolve every declaration, including unused ones, before table records
        // are considered. The fixed stack bounds reference depth independently
        // from the XML tree nesting limit.
        for root in 0..types.definitions.len() {
            if matches!(types.definitions[root].state, State::Resolved { .. }) {
                continue;
            }
            let mut path = [0; MAX_TYPE_DEPTH];
            let mut depth = 0;
            let mut current = root;
            let (mut domain, mut resolved_depth) = loop {
                check(budget, cancelled)?;
                match types.definitions[current].state {
                    State::Resolved { domain, depth } => break (domain, depth),
                    State::Visiting => return Err(TemplateError::Malformed),
                    State::Unseen => {}
                }
                if depth >= path.len() {
                    return Err(TemplateError::Unsupported);
                }
                path[depth] = current;
                depth += 1;
                types.definitions[current].state = State::Visiting;
                let base = types.definitions[current].base;
                if base.namespace == XSD {
                    break (Domain::builtin(&base.local)?, 0);
                }
                if !base.namespace.is_empty() {
                    return Err(TemplateError::Unsupported);
                }
                current = types
                    .find(&base.local, budget, cancelled)?
                    .ok_or(TemplateError::Unsupported)?;
            };
            while depth > 0 {
                check(budget, cancelled)?;
                depth -= 1;
                resolved_depth += 1;
                if resolved_depth > MAX_TYPE_DEPTH {
                    return Err(TemplateError::Unsupported);
                }
                let definition = &mut types.definitions[path[depth]];
                domain = definition.facets.apply(domain)?;
                definition.state = State::Resolved {
                    domain,
                    depth: resolved_depth,
                };
            }
        }
        Ok(types)
    }

    fn find(
        &self,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<usize>, TemplateError> {
        for (index, definition) in self.definitions.iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (name.len() + definition.name.len()) as u64)
                .map_err(ReadError::from)?;
            if definition.name == name {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    pub fn resolve(
        &self,
        name: &Name,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Domain, TemplateError> {
        check(budget, cancelled)?;
        if name.namespace == XSD {
            return Domain::builtin(&name.local);
        }
        if !name.namespace.is_empty() {
            return Err(TemplateError::Unsupported);
        }
        let index = self
            .find(&name.local, budget, cancelled)?
            .ok_or(TemplateError::Unsupported)?;
        match self.definitions[index].state {
            State::Resolved { domain, .. } => Ok(domain),
            _ => Err(TemplateError::Malformed),
        }
    }

    pub fn inline(
        &self,
        node: &Node,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Domain, TemplateError> {
        node.only_attributes(&[])?;
        let (base, facets) = restriction(node, budget, cancelled)?;
        facets.apply(self.resolve(base, budget, cancelled)?)
    }
}

fn restriction<'a>(
    node: &'a Node,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(&'a Name, Facets), TemplateError> {
    let restriction = single(node, "restriction")?;
    restriction.only_attributes(&["base"])?;
    let base = restriction
        .qualified("base")
        .ok_or(TemplateError::Malformed)?;
    let mut facets = Facets::default();
    for facet in children(restriction)? {
        check(budget, cancelled)?;
        facet.only_attributes(&["value"])?;
        if facet.name.namespace != XSD || !children(facet)?.is_empty() {
            return Err(TemplateError::Unsupported);
        }
        let value = facet.attribute("value").ok_or(TemplateError::Malformed)?;
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(TemplateError::Malformed);
        }
        let value = value.parse().map_err(|_| TemplateError::Unsupported)?;
        let slot = match facet.name.local.as_str() {
            "maxLength" => &mut facets.max_length,
            "totalDigits" => &mut facets.total_digits,
            "fractionDigits" => &mut facets.fraction_digits,
            _ => return Err(TemplateError::Unsupported),
        };
        if slot.replace(value).is_some() {
            return Err(TemplateError::Malformed);
        }
    }
    Ok((base, facets))
}
