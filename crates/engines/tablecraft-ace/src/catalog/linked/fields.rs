use super::metadata::OwnedText;
use crate::{InspectError, TableSchema, format::data_type, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

const SYSTEM_VARIABLE_TEXT_FLAGS: u8 = 0x12;
const FIELDS: &[(&str, Field, u8)] = &[
    ("Id", Field::Id, data_type::INT32),
    ("ParentId", Field::Parent, data_type::INT32),
    ("Flags", Field::Flags, data_type::INT32),
    ("Name", Field::Name, data_type::SHORT_TEXT),
    ("Type", Field::Type, data_type::INT16),
    ("Database", Field::Database, data_type::LONG_TEXT),
    ("ForeignName", Field::ForeignName, data_type::SHORT_TEXT),
    ("Connect", Field::Connect, data_type::LONG_TEXT),
];

#[derive(Clone, Copy)]
pub(super) enum Field {
    Id,
    Parent,
    Flags,
    Name,
    Type,
    Database,
    ForeignName,
    Connect,
}

pub(super) struct Fields {
    pub selected: Vec<(usize, Field)>,
    pub database: OwnedText,
    pub foreign_name: OwnedText,
    pub connect: OwnedText,
}

pub(super) fn select(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Fields, InspectError> {
    let mut fields = Fields {
        selected: Vec::new(),
        database: OwnedText::Missing,
        foreign_name: OwnedText::Missing,
        connect: OwnedText::Missing,
    };
    budget.charge_allocation(FIELDS.len() as u64, size_of::<(usize, Field)>() as u64)?;
    fields
        .selected
        .try_reserve_exact(FIELDS.len())
        .map_err(|_| InspectError::Allocation)?;
    for &(name, role, kind) in FIELDS {
        check(budget, cancelled)?;
        let mut matches = schema
            .columns()
            .iter()
            .enumerate()
            .filter(|(_, field)| field.name().eq_ignore_ascii_case(name));
        let candidate = matches.next();
        if matches.next().is_some() {
            return Err(InspectError::Malformed);
        }
        let required = matches!(
            role,
            Field::Id | Field::Parent | Field::Flags | Field::Name | Field::Type
        );
        let Some((position, column)) = candidate else {
            if required {
                return Err(InspectError::Malformed);
            }
            continue;
        };
        let expected_scalar = match role {
            Field::Id | Field::Parent | Field::Flags => ScalarType::Int32,
            Field::Type => ScalarType::Int16,
            _ => ScalarType::Text,
        };
        let qualified = column.native_type() == kind
            && column.scalar_type() == Some(expected_scalar)
            && (required
                || (!column.fixed()
                    && column.flags() == SYSTEM_VARIABLE_TEXT_FLAGS
                    && column.extended_flags() == 0));
        if qualified {
            fields.selected.push((position, role));
        } else if required {
            return Err(InspectError::Unsupported);
        } else {
            match role {
                Field::Database => fields.database = OwnedText::Unsupported,
                Field::ForeignName => fields.foreign_name = OwnedText::Unsupported,
                Field::Connect => fields.connect = OwnedText::Unsupported,
                _ => return Err(InspectError::Malformed),
            }
        }
    }
    fields
        .selected
        .sort_unstable_by_key(|(position, _)| *position);
    Ok(fields)
}
