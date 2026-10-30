use super::super::{super::TableExtraProperty, read as lexical};
use super::{
    DataMacroProgram, MacroProgramCoverage, MacroProgramDiagnostic as Diagnostic, MacroProgramNode,
    MacroProgramNodeKind as NodeKind, MacroProgramProfile,
    tree::{self, Kind, Name, Tree},
};
use crate::{InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

const MAX_PROGRAM_NODES: usize = 4096;
const MAX_ACTION_CHILDREN: usize = 64;

pub(super) fn inspect<'a>(
    property: &'a TableExtraProperty,
    profile: MacroProgramProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DataMacroProgram<'a>, InspectError> {
    let _span = tracing::debug_span!("native.table.extra.macro_program.inspect").entered();
    check(budget, cancelled)?;
    let xml = property.text().ok_or(InspectError::Unsupported)?;
    let envelopes = lexical::program_envelopes(xml, budget, cancelled)?;
    let tree = tree::scan(xml, envelopes.namespace().uri(), budget, cancelled)?;
    let mut builder = Builder {
        tree: &tree,
        definition: 0,
        nodes: Vec::new(),
        opaque: false,
    };

    match profile {
        MacroProgramProfile::DocumentedDesktop => match tree.elements[tree.root].kind {
            Kind::Definition => builder.definition(tree.root, budget, cancelled)?,
            Kind::Wrapper => {
                for child in tree.children(tree.root) {
                    builder.definition(child, budget, cancelled)?;
                }
            }
            _ => return Err(InspectError::Unsupported),
        },
    }
    if builder.definition != envelopes.definitions().len() {
        return Err(InspectError::Malformed);
    }
    check(budget, cancelled)?;
    let coverage = if builder.opaque {
        MacroProgramCoverage::ContainsOpaqueStructure
    } else {
        MacroProgramCoverage::RecognizedDesktopSubset
    };
    Ok(DataMacroProgram {
        property,
        xml,
        profile,
        envelopes,
        nodes: builder.nodes,
        coverage,
    })
}

struct Builder<'a> {
    tree: &'a Tree,
    definition: usize,
    nodes: Vec<MacroProgramNode>,
    opaque: bool,
}

impl Builder<'_> {
    fn push(
        &mut self,
        element: usize,
        parent: Option<usize>,
        kind: NodeKind,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, InspectError> {
        check(budget, cancelled)?;
        tree::reserve(&mut self.nodes, MAX_PROGRAM_NODES, budget)?;
        let index = self.nodes.len();
        self.opaque |= matches!(kind, NodeKind::Opaque { .. });
        self.nodes.push(MacroProgramNode {
            definition: self.definition,
            parent,
            span: self.tree.elements[element].span,
            kind,
        });
        Ok(index)
    }

    fn opaque(
        &mut self,
        element: usize,
        parent: Option<usize>,
        diagnostic: Diagnostic,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        self.push(
            element,
            parent,
            NodeKind::Opaque { diagnostic },
            budget,
            cancelled,
        )?;
        Ok(())
    }

    fn definition(
        &mut self,
        element: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        check(budget, cancelled)?;
        let tree = self.tree;
        if tree.elements[element].kind != Kind::Definition {
            return Err(InspectError::Unsupported);
        }
        let mut children = tree.children(element);
        match (children.next(), children.next()) {
            (Some(child), None) if !tree.elements[element].nonwhite_text => {
                if tree.elements[child].kind == Kind::Statements {
                    self.statements(child, None, budget, cancelled)?;
                } else {
                    self.opaque(child, None, diagnostic(tree, child), budget, cancelled)?;
                }
            }
            _ => self.opaque(
                element,
                None,
                Diagnostic::UnsupportedStructure,
                budget,
                cancelled,
            )?,
        }
        self.definition += 1;
        Ok(())
    }

    fn statements(
        &mut self,
        element: usize,
        parent: Option<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        check(budget, cancelled)?;
        let tree = self.tree;
        if !plain(tree, element) {
            return self.opaque(
                element,
                parent,
                diagnostic(tree, element),
                budget,
                cancelled,
            );
        }
        let parent = Some(self.push(element, parent, NodeKind::Statements, budget, cancelled)?);
        for child in tree.children(element) {
            check(budget, cancelled)?;
            match tree.elements[child].kind {
                Kind::Action => self.action(child, parent, budget, cancelled)?,
                Kind::ConditionalBlock => self.conditional(child, parent, budget, cancelled)?,
                _ => self.opaque(child, parent, diagnostic(tree, child), budget, cancelled)?,
            }
        }
        Ok(())
    }

    fn conditional(
        &mut self,
        element: usize,
        parent: Option<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        check(budget, cancelled)?;
        let tree = self.tree;
        let shape = conditional_shape(tree, element);
        let Some((branch, condition, statements)) = shape else {
            return self.opaque(
                element,
                parent,
                diagnostic(tree, element),
                budget,
                cancelled,
            );
        };
        let wrapper = self.push(
            element,
            parent,
            NodeKind::ConditionalBlock,
            budget,
            cancelled,
        )?;
        let branch = self.push(branch, Some(wrapper), NodeKind::If, budget, cancelled)?;
        self.push(
            condition,
            Some(branch),
            NodeKind::Condition {
                content: tree.elements[condition].content,
            },
            budget,
            cancelled,
        )?;
        self.statements(statements, Some(branch), budget, cancelled)
    }

    fn action(
        &mut self,
        element: usize,
        parent: Option<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        check(budget, cancelled)?;
        let tree = self.tree;
        // Even an unknown action's child sequence is bounded and paid.
        for (position, _) in tree.children(element).enumerate() {
            check(budget, cancelled)?;
            if position == MAX_ACTION_CHILDREN {
                return Err(InspectError::StructureLimit);
            }
        }
        let action = &tree.elements[element];
        let reason = if action.attributes.foreign {
            Some(Diagnostic::ForeignAttribute)
        } else if action.attributes.extra {
            Some(Diagnostic::UnknownAttribute)
        } else if action.nonwhite_text {
            Some(Diagnostic::UnsupportedStructure)
        } else {
            match action.attributes.name {
                Some(Name::RaiseError) => None,
                Some(Name::SetField) => Some(Diagnostic::UnsupportedAction),
                Some(_) => Some(Diagnostic::UnknownAction),
                None => Some(Diagnostic::UnsupportedStructure),
            }
        };
        if let Some(reason) = reason {
            return self.opaque(element, parent, reason, budget, cancelled);
        }

        let mut number = None;
        let mut description = None;
        for child in tree.children(element) {
            check(budget, cancelled)?;
            let argument = &tree.elements[child];
            let reason = if argument.kind == Kind::Foreign {
                Some(Diagnostic::ForeignElement)
            } else if argument.attributes.foreign {
                Some(Diagnostic::ForeignAttribute)
            } else if argument.attributes.extra {
                Some(Diagnostic::UnknownAttribute)
            } else if argument.kind != Kind::Argument || argument.first.is_some() {
                Some(Diagnostic::UnsupportedStructure)
            } else {
                match argument.attributes.name {
                    Some(Name::Number) if number.replace(argument.content).is_some() => {
                        Some(Diagnostic::DuplicateArgument)
                    }
                    Some(Name::Description) if description.replace(argument.content).is_some() => {
                        Some(Diagnostic::DuplicateArgument)
                    }
                    Some(Name::Number | Name::Description) => None,
                    _ => Some(Diagnostic::UnknownArgument),
                }
            };
            if let Some(reason) = reason {
                return self.opaque(element, parent, reason, budget, cancelled);
            }
        }
        let Some(number) = number else {
            return self.opaque(
                element,
                parent,
                Diagnostic::MissingArgument,
                budget,
                cancelled,
            );
        };
        self.push(
            element,
            parent,
            NodeKind::RaiseError {
                number,
                description,
            },
            budget,
            cancelled,
        )?;
        Ok(())
    }
}

fn plain(tree: &Tree, element: usize) -> bool {
    let element = &tree.elements[element];
    element.kind != Kind::Foreign && element.attributes.plain() && !element.nonwhite_text
}

fn diagnostic(tree: &Tree, element: usize) -> Diagnostic {
    let element = &tree.elements[element];
    if element.kind == Kind::Foreign {
        Diagnostic::ForeignElement
    } else if element.attributes.foreign {
        Diagnostic::ForeignAttribute
    } else if element.attributes.extra || element.attributes.name.is_some() {
        Diagnostic::UnknownAttribute
    } else {
        Diagnostic::UnsupportedStructure
    }
}

fn conditional_shape(tree: &Tree, element: usize) -> Option<(usize, usize, usize)> {
    if !plain(tree, element) {
        return None;
    }
    let mut children = tree.children(element);
    let branch = children.next()?;
    if children.next().is_some() || tree.elements[branch].kind != Kind::If || !plain(tree, branch) {
        return None;
    }
    let mut children = tree.children(branch);
    let condition = children.next()?;
    let statements = children.next()?;
    if children.next().is_some()
        || tree.elements[condition].kind != Kind::Condition
        || !tree.elements[condition].attributes.plain()
        || tree.elements[condition].first.is_some()
        || tree.elements[statements].kind != Kind::Statements
        || !plain(tree, statements)
    {
        return None;
    }
    Some((branch, condition, statements))
}
