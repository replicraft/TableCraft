//! Explicit structural program reports; no expression or argument text is printed.
use super::{Error, line};
use crate::output::TableOutput;
use tablecraft_ace::{
    DataMacroProgram, InspectError, MacroProgramCoverage, MacroProgramNodeKind,
    MacroProgramProfile, TableExtra, XmlByteSpan,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Inspected<'a> {
    group: u16,
    property: &'a str,
    program: DataMacroProgram<'a>,
}

pub(super) fn report(
    extra: &TableExtra,
    output: &mut TableOutput,
    budget: &mut ResourceBudget,
) -> Result<(), Error> {
    let mut count = 0_usize;
    for group in extra.groups() {
        step(budget)?;
        count = count
            .checked_add(
                group
                    .properties()
                    .ok_or(Error::Read(InspectError::Unsupported))?
                    .len(),
            )
            .ok_or(Error::Read(InspectError::StructureLimit))?;
    }
    budget
        .charge_allocation(count as u64, size_of::<Inspected<'_>>() as u64)
        .map_err(InspectError::from)
        .map_err(Error::Read)?;
    let mut programs = Vec::new();
    programs
        .try_reserve_exact(count)
        .map_err(|_| Error::Read(InspectError::Allocation))?;
    let mut definitions = 0_usize;
    let mut nodes = 0_usize;
    for group in extra.groups() {
        for property in group
            .properties()
            .ok_or(Error::Read(InspectError::Unsupported))?
        {
            step(budget)?;
            let program = property
                .inspect_macro_program(MacroProgramProfile::DocumentedDesktop, budget, &|| false)
                .map_err(Error::Read)?;
            definitions = definitions
                .checked_add(program.envelopes().definitions().len())
                .ok_or(Error::Read(InspectError::StructureLimit))?;
            nodes = nodes
                .checked_add(program.nodes().len())
                .ok_or(Error::Read(InspectError::StructureLimit))?;
            programs.push(Inspected {
                group: group.kind(),
                property: property.name(),
                program,
            });
        }
    }

    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Program structural profile",
            "Documented desktop Statements/If/RaiseError",
        ],
    )?;
    line(
        output,
        &[
            "Program span coordinates",
            "Original decoded XML UTF-8 bytes; BOM retained",
        ],
    )?;
    line(output, &["Program execution", "Not provided"])?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Group kind",
            "Property",
            "Definition",
            "XML namespace",
            "Event",
            "Macro name",
            "Document structural coverage",
        ],
    )?;
    for inspected in &programs {
        for (index, definition) in inspected
            .program
            .envelopes()
            .definitions()
            .iter()
            .enumerate()
        {
            step(budget)?;
            line(
                output,
                &[
                    &inspected.group.to_string(),
                    inspected.property,
                    &index.to_string(),
                    inspected.program.envelopes().namespace().uri(),
                    definition.event().unwrap_or(""),
                    definition.name().unwrap_or(""),
                    match inspected.program.coverage() {
                        MacroProgramCoverage::RecognizedDesktopSubset => {
                            "Recognized desktop subset; expressions unevaluated"
                        }
                        MacroProgramCoverage::ContainsOpaqueStructure => {
                            "Contains opaque structure; expressions unevaluated"
                        }
                    },
                ],
            )?;
        }
    }

    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Group kind",
            "Property",
            "Definition",
            "Node",
            "Parent",
            "Kind",
            "XML start",
            "XML end",
            "Condition start",
            "Condition end",
            "Number start",
            "Number end",
            "Description start",
            "Description end",
            "Opaque diagnostic",
        ],
    )?;
    for inspected in &programs {
        for (index, node) in inspected.program.nodes().iter().enumerate() {
            step(budget)?;
            let (kind, condition, number, description, diagnostic) = match node.kind() {
                MacroProgramNodeKind::Statements => ("Statements", None, None, None, String::new()),
                MacroProgramNodeKind::ConditionalBlock => {
                    ("ConditionalBlock", None, None, None, String::new())
                }
                MacroProgramNodeKind::If => ("If", None, None, None, String::new()),
                MacroProgramNodeKind::Condition { content } => {
                    ("Condition", Some(content), None, None, String::new())
                }
                MacroProgramNodeKind::RaiseError {
                    number,
                    description,
                } => ("RaiseError", None, Some(number), description, String::new()),
                MacroProgramNodeKind::Opaque { diagnostic } => {
                    ("Opaque", None, None, None, format!("{diagnostic:?}"))
                }
            };
            let (condition_start, condition_end) = coordinates(condition);
            let (number_start, number_end) = coordinates(number);
            let (description_start, description_end) = coordinates(description);
            line(
                output,
                &[
                    &inspected.group.to_string(),
                    inspected.property,
                    &node.definition().to_string(),
                    &index.to_string(),
                    &node
                        .parent()
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    kind,
                    &node.span().start().to_string(),
                    &node.span().end().to_string(),
                    &condition_start,
                    &condition_end,
                    &number_start,
                    &number_end,
                    &description_start,
                    &description_end,
                    &diagnostic,
                ],
            )?;
        }
    }
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &["Program documents inspected", &programs.len().to_string()],
    )?;
    line(output, &["Program definitions", &definitions.to_string()])?;
    line(output, &["Program nodes", &nodes.to_string()])?;
    line(
        output,
        &[
            "Program authority",
            if programs.is_empty() {
                "No program documents inspected; absence not certified"
            } else {
                "Structural inspection only; execution and absence not certified"
            },
        ],
    )?;
    Ok(())
}

fn coordinates(span: Option<XmlByteSpan>) -> (String, String) {
    span.map(|value| (value.start().to_string(), value.end().to_string()))
        .unwrap_or_default()
}

fn step(budget: &mut ResourceBudget) -> Result<(), Error> {
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)
        .map_err(Error::Read)
}
