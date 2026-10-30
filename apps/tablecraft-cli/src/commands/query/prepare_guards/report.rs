use super::Error;
use crate::output::{self, OutputError, TableOutput};
use tablecraft_access::engineering::guards::GuardPreparationSummary;
use tablecraft_ace::{BeforeEvent, BeforeGuardTraceKind, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

const HEADER_ALLOWANCE: usize = 1024;
const TRACE_ROW_ALLOWANCE: usize = 256;
const ESCAPED_TEXT_FACTOR: usize = 8;

pub(super) fn format(
    prepared: &GuardPreparationSummary,
    trace: bool,
    budget: &mut ResourceBudget,
) -> Result<String, Error> {
    let description_bytes = prepared
        .rejection()
        .and_then(|value| value.description())
        .map_or(0, str::len);
    let name_bytes = prepared
        .properties()
        .iter()
        .try_fold(0_usize, |total, property| {
            total.checked_add(property.name().len())
        })
        .ok_or(Error::Output(OutputError::Limit))?;
    let estimate = description_bytes
        .checked_add(name_bytes)
        .and_then(|text| text.checked_mul(ESCAPED_TEXT_FACTOR))
        .and_then(|description| HEADER_ALLOWANCE.checked_add(description))
        .and_then(|header| {
            prepared
                .properties()
                .len()
                .checked_mul(TRACE_ROW_ALLOWANCE)
                .and_then(|rows| header.checked_add(rows))
        })
        .and_then(|header| {
            if trace {
                prepared
                    .trace()
                    .len()
                    .checked_mul(TRACE_ROW_ALLOWANCE)
                    .and_then(|rows| header.checked_add(rows))
            } else {
                Some(header)
            }
        })
        .filter(|bytes| *bytes <= output::MAX_OUTPUT_BYTES)
        .ok_or(Error::Output(OutputError::Limit))?;
    budget
        .charge_allocation(estimate as u64, 2)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    budget
        .charge(Resource::Steps, estimate as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let mut result = TableOutput::new();
    line(
        &mut result,
        "Guard preparation",
        "Explicit caller-selected property subset; not detected native macro coverage or a save permit",
    )?;
    line(
        &mut result,
        "Guard order",
        "Caller order; native dispatch order unqualified",
    )?;
    line(
        &mut result,
        "If policy",
        "Boolean only; NULL and nonBoolean refuse",
    )?;
    line(
        &mut result,
        "Updated policy",
        "Provisional actual-value-change; native timing unqualified",
    )?;
    line(
        &mut result,
        "Event",
        match prepared.event() {
            BeforeEvent::Insert => "BeforeChange / explicit insert",
            BeforeEvent::Update => "BeforeChange / update",
            BeforeEvent::Delete => "BeforeDelete / Old-only",
        },
    )?;
    line(
        &mut result,
        "Guard decision",
        if prepared.rejection().is_some() {
            "Rejected"
        } else {
            "Passed subset"
        },
    )?;
    number(
        &mut result,
        "Selected intent rows",
        prepared.selected_rows(),
    )?;
    number(&mut result, "Property witnesses", prepared.program_count())?;
    number(&mut result, "Visited steps", prepared.trace().len())?;
    if let Some(rejection) = prepared.rejection() {
        result.display(&"Error Number").map_err(Error::Output)?;
        result.separator().map_err(Error::Output)?;
        result.display(&rejection.number()).map_err(Error::Output)?;
        result.newline().map_err(Error::Output)?;
        result
            .display(&"Error Description")
            .map_err(Error::Output)?;
        result.separator().map_err(Error::Output)?;
        if let Some(description) = rejection.description() {
            result.text_value(description).map_err(Error::Output)?;
        } else {
            result.display(&"Absent").map_err(Error::Output)?;
        }
        result.newline().map_err(Error::Output)?;
    }
    line(&mut result, "Candidate", "Not created")?;
    line(
        &mut result,
        "Normal save / macro absence",
        "Not authorized / not certified",
    )?;
    result.table_section().map_err(Error::Output)?;
    result
        .display(&"Guard order\tGroup kind\tSelected property")
        .map_err(Error::Output)?;
    result.newline().map_err(Error::Output)?;
    for (index, property) in prepared.properties().iter().enumerate() {
        result.display(&index).map_err(Error::Output)?;
        result.separator().map_err(Error::Output)?;
        result.display(&property.group()).map_err(Error::Output)?;
        result.separator().map_err(Error::Output)?;
        result.text_value(property.name()).map_err(Error::Output)?;
        result.newline().map_err(Error::Output)?;
    }
    if trace {
        line(
            &mut result,
            "Span coordinates",
            "Original decoded XML UTF-8 bytes; BOM retained",
        )?;
        result.table_section().map_err(Error::Output)?;
        result
            .display(&"Step\tKind\tRow\tProgram\tNode\tXML start\tXML end")
            .map_err(Error::Output)?;
        result.newline().map_err(Error::Output)?;
        for (index, step) in prepared.trace().iter().enumerate() {
            result.display(&index).map_err(Error::Output)?;
            result.separator().map_err(Error::Output)?;
            let kind = match step.kind() {
                BeforeGuardTraceKind::CompiledAll => "CompiledAll",
                BeforeGuardTraceKind::Begin => "Begin",
                BeforeGuardTraceKind::Condition(true) => "If true",
                BeforeGuardTraceKind::Condition(false) => "If false",
                BeforeGuardTraceKind::End => "End",
                BeforeGuardTraceKind::Raised(_) => "RaiseError",
                BeforeGuardTraceKind::Passed => "Passed",
                BeforeGuardTraceKind::Rejected => "Rejected",
            };
            result.display(&kind).map_err(Error::Output)?;
            for value in [
                step.row(),
                step.program(),
                step.node(),
                step.source_span().map(|span| span.start()),
                step.source_span().map(|span| span.end()),
            ] {
                result.separator().map_err(Error::Output)?;
                if let Some(value) = value {
                    result.display(&value).map_err(Error::Output)?;
                }
            }
            result.newline().map_err(Error::Output)?;
        }
    }
    let text = result.finish();
    budget
        .charge(Resource::Bytes, text.len() as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    Ok(text)
}

fn line(output: &mut TableOutput, key: &str, value: &str) -> Result<(), Error> {
    output.display(&key).map_err(Error::Output)?;
    output.separator().map_err(Error::Output)?;
    output.display(&value).map_err(Error::Output)?;
    output.newline().map_err(Error::Output)
}

fn number(output: &mut TableOutput, key: &str, value: usize) -> Result<(), Error> {
    output.display(&key).map_err(Error::Output)?;
    output.separator().map_err(Error::Output)?;
    output.display(&value).map_err(Error::Output)?;
    output.newline().map_err(Error::Output)
}
