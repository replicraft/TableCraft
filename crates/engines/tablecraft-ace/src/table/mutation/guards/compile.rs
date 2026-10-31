use super::{
    BeforeEvent, BeforeEventGuardError, BeforeEventGuardSelector, BeforeGuardProgramRevision,
    BeforeGuardUnsupported, PreparationProfile, buffer, expression, selection, source,
};
use crate::{
    InspectError, MacroProgramNodeKind, MacroProgramProfile, MacroProgramTextKind, NativeImage,
    PayloadPresence, TableExtraCoverage, TableSnapshot, XmlByteSpan, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::evaluation::EvalLimits;
use tablecraft_sql::select::Expression;

const MAX_INSTRUCTIONS: usize = 4096;
const MAX_DESCRIPTION_BYTES: usize = EvalLimits::DEFAULT_TEXT_BYTES;
// Explicit native MR2 carrier profile; no broader property-flag meaning inferred.
const BEFORE_CHANGE_PROPERTY_FLAGS: u8 = 0x01;

pub(super) enum Description {
    Literal(String),
    Expression(Expression),
}

pub(super) enum Operation {
    SetField {
        column: usize,
        expression: Expression,
    },
    If {
        expression: Expression,
        end: usize,
    },
    Raise {
        number: Expression,
        description: Option<Description>,
    },
}

pub(super) struct Instruction {
    pub program: usize,
    pub definition: usize,
    pub node: usize,
    pub span: XmlByteSpan,
    pub operation: Operation,
}

pub(super) struct Compiled {
    pub instructions: Vec<Instruction>,
    pub programs: Vec<BeforeGuardProgramRevision>,
}

pub(super) fn derive(
    image: &NativeImage,
    table: &TableSnapshot,
    selectors: &[BeforeEventGuardSelector<'_>],
    event: BeforeEvent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Compiled, BeforeEventGuardError> {
    derive_for(
        image,
        table,
        selectors,
        event,
        PreparationProfile::Guards,
        budget,
        cancelled,
    )
}

pub(super) fn derive_fields(
    image: &NativeImage,
    table: &TableSnapshot,
    selectors: &[BeforeEventGuardSelector<'_>],
    event: BeforeEvent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Compiled, BeforeEventGuardError> {
    if event == BeforeEvent::Delete {
        return Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::Context,
        ));
    }
    derive_for(
        image,
        table,
        selectors,
        event,
        PreparationProfile::BeforeChangeFields,
        budget,
        cancelled,
    )
}

fn derive_for(
    image: &NativeImage,
    table: &TableSnapshot,
    selectors: &[BeforeEventGuardSelector<'_>],
    event: BeforeEvent,
    profile: PreparationProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Compiled, BeforeEventGuardError> {
    selection::validate(selectors, budget, cancelled)?;
    let extra = image.inspect_table_extra(table.schema().object_id(), budget, cancelled)?;
    if extra.presence() != PayloadPresence::Present
        || extra.coverage() != TableExtraCoverage::WideMr2
    {
        return Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::Structure,
        ));
    }
    if profile == PreparationProfile::BeforeChangeFields {
        selection::require_complete(&extra, selectors, budget, cancelled)?;
    }
    let mut instructions = Vec::new();
    let mut programs = Vec::new();
    let mut selected = 0_usize;
    for selector in selectors {
        check(budget, cancelled)?;
        let selected_property = selection::resolve(&extra, *selector, budget, cancelled)?;
        let group = &extra.groups()[selected_property.group];
        let property = selected_property.value;
        check(budget, cancelled)?;
        if profile == PreparationProfile::BeforeChangeFields
            && (property.native_type() != crate::format::data_type::LONG_TEXT
                || property.flags() != BEFORE_CHANGE_PROPERTY_FLAGS)
        {
            return Err(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Structure,
            ));
        }
        let program = property.inspect_macro_program(
            match profile {
                PreparationProfile::Guards => MacroProgramProfile::DocumentedDesktop,
                PreparationProfile::BeforeChangeFields => MacroProgramProfile::BeforeChangeFields,
            },
            budget,
            cancelled,
        )?;
        let program_index = programs.len();
        let mut source = buffer::vector(property.raw().len(), budget, cancelled)?;
        source.extend_from_slice(property.raw());
        let mut name = buffer::vector(property.name().len(), budget, cancelled)?;
        name.extend_from_slice(property.name().as_bytes());
        let name = String::from_utf8(name).map_err(|_| InspectError::Malformed)?;
        buffer::push(
            &mut programs,
            BeforeGuardProgramRevision {
                group: group.kind(),
                group_ordinal: selected_property.group,
                property: selected_property.property,
                name,
                catalog_page: extra.catalog_page(),
                catalog_slot: extra.catalog_slot(),
                raw: source,
            },
            NativeImage::MAX_BEFORE_GUARD_PROPERTIES,
            budget,
            cancelled,
        )?;

        for (definition, envelope) in program.envelopes().definitions().iter().enumerate() {
            check(budget, cancelled)?;
            let Some(declared) = envelope.event() else {
                return Err(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Context,
                ));
            };
            if !matches!(declared, "BeforeChange" | "BeforeDelete") {
                return Err(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Context,
                ));
            }
            if declared != source::event_name(event) {
                return Err(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Context,
                ));
            }
            selected += 1;
            for (node_index, node) in program.nodes().iter().enumerate() {
                check(budget, cancelled)?;
                if node.definition() != definition {
                    continue;
                }
                let operation = match node.kind() {
                    MacroProgramNodeKind::Opaque { .. } => {
                        return Err(BeforeEventGuardError::Unsupported(
                            BeforeGuardUnsupported::Structure,
                        ));
                    }
                    MacroProgramNodeKind::Condition { .. } => {
                        let text = program
                            .text_content(
                                node_index,
                                MacroProgramTextKind::Condition,
                                budget,
                                cancelled,
                            )?
                            .ok_or(InspectError::Malformed)?;
                        let expression = expression::compile(
                            text.text(),
                            event,
                            table,
                            profile,
                            budget,
                            cancelled,
                        )?;
                        let parent = program
                            .nodes()
                            .get(node.parent().ok_or(InspectError::Malformed)?)
                            .ok_or(InspectError::Malformed)?;
                        Operation::If {
                            expression,
                            end: parent.span().end(),
                        }
                    }
                    MacroProgramNodeKind::SetField { .. } => {
                        let target = program
                            .text_content(
                                node_index,
                                MacroProgramTextKind::Field,
                                budget,
                                cancelled,
                            )?
                            .ok_or(InspectError::Malformed)?;
                        let value = program
                            .text_content(
                                node_index,
                                MacroProgramTextKind::Value,
                                budget,
                                cancelled,
                            )?
                            .ok_or(InspectError::Malformed)?;
                        let column = expression::target(target.text(), table, budget, cancelled)?;
                        let expression = expression::compile(
                            value.text().strip_prefix('=').unwrap_or(value.text()),
                            event,
                            table,
                            profile,
                            budget,
                            cancelled,
                        )?;
                        Operation::SetField { column, expression }
                    }
                    MacroProgramNodeKind::RaiseError { .. } => {
                        let number = program
                            .text_content(
                                node_index,
                                MacroProgramTextKind::Number,
                                budget,
                                cancelled,
                            )?
                            .ok_or(InspectError::Malformed)?;
                        let number = expression::compile(
                            number.text().strip_prefix('=').unwrap_or(number.text()),
                            event,
                            table,
                            profile,
                            budget,
                            cancelled,
                        )?;
                        let description = program
                            .text_content(
                                node_index,
                                MacroProgramTextKind::Description,
                                budget,
                                cancelled,
                            )?
                            .map(|text| {
                                if let Some(value) = text.text().strip_prefix('=') {
                                    expression::compile(
                                        value, event, table, profile, budget, cancelled,
                                    )
                                    .map(Description::Expression)
                                } else {
                                    if text.text().len() > MAX_DESCRIPTION_BYTES {
                                        return Err(InspectError::StructureLimit.into());
                                    }
                                    let mut value =
                                        buffer::vector(text.text().len(), budget, cancelled)?;
                                    value.extend_from_slice(text.text().as_bytes());
                                    String::from_utf8(value)
                                        .map(Description::Literal)
                                        .map_err(|_| InspectError::Malformed.into())
                                }
                            })
                            .transpose()?;
                        Operation::Raise {
                            number,
                            description,
                        }
                    }
                    _ => continue,
                };
                buffer::push(
                    &mut instructions,
                    Instruction {
                        program: program_index,
                        definition,
                        node: node_index,
                        span: node.span(),
                        operation,
                    },
                    MAX_INSTRUCTIONS,
                    budget,
                    cancelled,
                )?;
            }
        }
    }
    if selected == 0 {
        return Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::Context,
        ));
    }
    check(budget, cancelled)?;
    Ok(Compiled {
        instructions,
        programs,
    })
}
