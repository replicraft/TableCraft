use super::Error;
use crate::{
    native::publication,
    output::{self, OutputError, TableOutput},
};
use tablecraft_ace::{Ace16BeforeChangePreview, BeforeGuardTraceKind, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

const BASE_ALLOWANCE: usize = 1024;
const METADATA_ROW_ALLOWANCE: usize = 256;
const ESCAPED_TEXT_FACTOR: usize = 8;

pub(super) fn format(
    candidate: &Ace16BeforeChangePreview,
    matched: usize,
    trace: bool,
    required: bool,
    budget: &mut ResourceBudget,
) -> Result<String, Error> {
    let names = candidate
        .programs()
        .iter()
        .try_fold(0_usize, |total, program| {
            total.checked_add(program.name().len())
        })
        .ok_or(Error::Output(OutputError::Limit))?;
    let rows = candidate
        .programs()
        .len()
        .checked_add(if trace {
            candidate
                .trace()
                .len()
                .checked_add(candidate.assignments().len())
                .ok_or(Error::Output(OutputError::Limit))?
        } else {
            0
        })
        .ok_or(Error::Output(OutputError::Limit))?;
    let estimate = names
        .checked_mul(ESCAPED_TEXT_FACTOR)
        .and_then(|text| {
            rows.checked_mul(METADATA_ROW_ALLOWANCE)
                .and_then(|rows| text.checked_add(rows))
        })
        .and_then(|rows| rows.checked_add(BASE_ALLOWANCE))
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

    let mut output = TableOutput::new();
    line(
        &mut output,
        "Engineering copy",
        "Explicit selected BeforeChange subset; native dispatch, required coverage and normal saves unqualified",
    )?;
    line(
        &mut output,
        "Program order",
        "Caller order; no automatic discovery",
    )?;
    number(&mut output, "Matched rows", matched)?;
    number(&mut output, "Changed rows", candidate.changed_rows())?;
    number(&mut output, "Selected programs", candidate.programs().len())?;
    number(&mut output, "Visited steps", candidate.trace().len())?;
    number(
        &mut output,
        "SetField assignments",
        candidate.assignments().len(),
    )?;
    if required {
        output
            .display(&publication::REQUIRED_REPORT)
            .map_err(Error::Output)?;
    }

    output
        .display(&"Program\tGroup kind\tSelected property")
        .map_err(Error::Output)?;
    output.newline().map_err(Error::Output)?;
    for (index, program) in candidate.programs().iter().enumerate() {
        output.display(&index).map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output.display(&program.group()).map_err(Error::Output)?;
        output.separator().map_err(Error::Output)?;
        output.text_value(program.name()).map_err(Error::Output)?;
        output.newline().map_err(Error::Output)?;
    }
    if trace {
        line(
            &mut output,
            "Span coordinates",
            "Original decoded XML UTF-8 bytes; not database offsets",
        )?;
        output
            .display(&"Step\tKind\tRow\tProgram\tNode\tXML start\tXML end")
            .map_err(Error::Output)?;
        output.newline().map_err(Error::Output)?;
        for (index, step) in candidate.trace().iter().enumerate() {
            output.display(&index).map_err(Error::Output)?;
            output.separator().map_err(Error::Output)?;
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
            output.display(&kind).map_err(Error::Output)?;
            for value in [
                step.row(),
                step.program(),
                step.node(),
                step.source_span().map(|span| span.start()),
                step.source_span().map(|span| span.end()),
            ] {
                output.separator().map_err(Error::Output)?;
                if let Some(value) = value {
                    output.display(&value).map_err(Error::Output)?;
                }
            }
            output.newline().map_err(Error::Output)?;
        }
        output
            .display(&"Assignment\tBefore step\tRow\tColumn\tProgram\tNode\tXML start\tXML end")
            .map_err(Error::Output)?;
        output.newline().map_err(Error::Output)?;
        for (index, assignment) in candidate.assignments().iter().enumerate() {
            output.display(&index).map_err(Error::Output)?;
            for value in [
                assignment.trace_position(),
                assignment.ordinal(),
                assignment.column(),
                assignment.program(),
                assignment.node(),
                assignment.source_span().start(),
                assignment.source_span().end(),
            ] {
                output.separator().map_err(Error::Output)?;
                output.display(&value).map_err(Error::Output)?;
            }
            output.newline().map_err(Error::Output)?;
        }
    }
    let text = output.finish();
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
