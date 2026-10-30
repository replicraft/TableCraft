use super::{MAX_LONG_VALUE_BYTES, PARSER_ALLOCATION_FACTOR, reference_character};
use crate::{InspectError, header::check};
use quick_xml::{Reader, events::Event};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Materialize only a recognized leaf's already-validated inner XML. This is
/// private to the XML owner; callers cannot submit arbitrary fragment ranges.
pub(in super::super) fn content(
    raw: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, InspectError> {
    check(budget, cancelled)?;
    if raw.len() > MAX_LONG_VALUE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    budget.charge(Resource::Steps, raw.len() as u64)?;
    budget.charge_allocation(raw.len() as u64, PARSER_ALLOCATION_FACTOR)?;
    budget.charge_allocation(raw.len() as u64, 1)?;

    let mut output = String::new();
    output
        .try_reserve_exact(raw.len())
        .map_err(|_| InspectError::Allocation)?;

    // A fragment-leading FEFF is ordinary element text. Reader otherwise treats
    // it as a document BOM, so preserve all such characters before parsing.
    let input = raw.trim_start_matches('\u{feff}');
    let leading = raw.len() - input.len();
    budget.charge(Resource::Steps, leading as u64)?;
    output.push_str(&raw[..leading]);

    let mut reader = Reader::from_str(input);
    reader.config_mut().check_comments = true;
    loop {
        check(budget, cancelled)?;
        let before = reader.buffer_position();
        let event = reader.read_event().map_err(|_| InspectError::Malformed)?;
        budget.charge(Resource::Steps, reader.buffer_position() - before)?;

        match event {
            Event::Text(text) => output.push_str(&text.xml10_content()),
            Event::CData(text) => output.push_str(&text.xml10_content()),
            Event::GeneralRef(reference) => output.push(reference_character(&reference)?),
            Event::Comment(_) => {}
            Event::Eof => break,
            // A recognized leaf has no nested elements. Never strip foreign or
            // unknown markup, declarations, DTDs or processing instructions.
            _ => return Err(InspectError::Unsupported),
        }
    }

    check(budget, cancelled)?;
    Ok(output)
}
