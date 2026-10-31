//! Observed local-table LvExtra -> MR2 property -> XML definition joins.
use super::super::{BeforeEventGuardError, BeforeGuardUnsupported, buffer};
use super::{unresolved, unsupported_event};
use crate::{
    InspectError, MacroPropertyObservation, NativeImage, PayloadPresence, TableExtraCoverage,
    TableMacroCoverage, header::check,
};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
};

const EVENT_GROUP: u16 = 3;
const NAMED_GROUP: u16 = 4;
const PROPERTY_FLAGS: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Event {
    BeforeChange,
    BeforeDelete,
    AfterInsert,
    AfterUpdate,
    AfterDelete,
    Named,
}

pub(super) struct Reference {
    pub group: usize,
    pub property: usize,
    pub event: Event,
    pub named: Option<String>,
}

pub(super) struct Bound<'image> {
    pub coverage: TableMacroCoverage<'image>,
    pub references: Vec<Reference>,
    pub unresolved: bool,
}

fn named_copy(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, InspectError> {
    budget.charge(Resource::Steps, name.len() as u64)?;
    let mut bytes = buffer::vector(name.len(), budget, cancelled)?;
    bytes.extend_from_slice(name.as_bytes());
    String::from_utf8(bytes).map_err(|_| InspectError::Malformed)
}

pub(super) fn capture<'image>(
    image: &'image NativeImage,
    table: ObjectId,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound<'image>, BeforeEventGuardError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    let coverage = image.inspect_table_macro_coverage(table, budget, cancelled)?;
    if coverage.table_origin().object_id() != table || coverage.table_extra().object_id() != table {
        return Err(BeforeEventGuardError::Preservation);
    }
    let mut unresolved_carriers = coverage.body().presence() != PayloadPresence::Absent
        || coverage.module().presence() != PayloadPresence::Absent
        || !coverage.children().is_empty();
    let extra = coverage.table_extra();
    if extra.presence() != PayloadPresence::Present
        || extra.coverage() != TableExtraCoverage::WideMr2
    {
        check(budget, cancelled)?;
        return Ok(Bound {
            coverage,
            references: Vec::new(),
            unresolved: true,
        });
    }
    let inventory = extra.inspect_macro_inventory(budget, cancelled)?;
    unresolved_carriers |= !inventory.opaque_groups().is_empty()
        || !inventory.unrepresented_names().is_empty()
        || inventory.properties().is_empty();
    if inventory.properties().len() > NativeImage::MAX_BEFORE_GUARD_PROPERTIES {
        return Err(InspectError::StructureLimit.into());
    }
    let mut references: Vec<Reference> =
        buffer::vector(inventory.properties().len(), budget, cancelled)?;
    for observed in inventory.properties() {
        check(budget, cancelled)?;
        let group_index = observed.group_ordinal();
        let property_index = observed.property_ordinal();
        let group = extra.groups().get(group_index).ok_or_else(unresolved)?;
        let property = group
            .properties()
            .and_then(|properties| properties.get(property_index))
            .ok_or_else(unresolved)?;
        if property.native_type() != crate::format::data_type::LONG_TEXT
            || property.flags() != PROPERTY_FLAGS
        {
            unresolved_carriers = true;
            continue;
        }
        let MacroPropertyObservation::Envelopes(xml) = observed.observation() else {
            unresolved_carriers = true;
            continue;
        };
        // Each observed native property owns exactly one definition. A general
        // XML wrapper containing multiple declarations is not this native form.
        let [definition] = xml.definitions() else {
            unresolved_carriers = true;
            continue;
        };
        let (event, named) = match (definition.event(), definition.name()) {
            (Some(event), None) if group.kind() == EVENT_GROUP => {
                let event = match event {
                    "BeforeChange" => Event::BeforeChange,
                    "BeforeDelete" => Event::BeforeDelete,
                    "AfterInsert" => Event::AfterInsert,
                    "AfterUpdate" => Event::AfterUpdate,
                    "AfterDelete" => Event::AfterDelete,
                    _ => return Err(unsupported_event()),
                };
                (event, None)
            }
            (None, Some(name)) if group.kind() == NAMED_GROUP => {
                (Event::Named, Some(named_copy(name, budget, cancelled)?))
            }
            _ => return Err(unresolved()),
        };
        for prior in &references {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            let duplicate = if event == Event::Named && prior.event == Event::Named {
                let left = named.as_deref().ok_or_else(unresolved)?;
                let right = prior.named.as_deref().ok_or_else(unresolved)?;
                budget
                    .charge(Resource::Steps, left.len().max(right.len()) as u64)
                    .map_err(InspectError::from)?;
                left == right
            } else {
                event == prior.event
            };
            if duplicate {
                return Err(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::NativeEventConflict,
                ));
            }
        }
        references.push(Reference {
            group: group_index,
            property: property_index,
            event,
            named,
        });
    }
    drop(inventory);
    check(budget, cancelled)?;
    Ok(Bound {
        coverage,
        references,
        unresolved: unresolved_carriers,
    })
}
