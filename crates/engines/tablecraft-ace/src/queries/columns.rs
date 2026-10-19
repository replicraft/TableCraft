use crate::{InspectError, TableSnapshot, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) const ATTRIBUTE: usize = 0;
pub(super) const EXPRESSION: usize = 1;
pub(super) const FLAG: usize = 2;
pub(super) const EXTRA: usize = 3;
pub(super) const NAME1: usize = 4;
pub(super) const NAME2: usize = 5;
pub(super) const OBJECT_ID: usize = 6;
pub(super) const ORDER: usize = 7;
pub(super) const FIELD_COUNT: usize = 8;
const FIELDS: [(&str, ScalarType); FIELD_COUNT] = [
    ("Attribute", ScalarType::Byte),
    ("Expression", ScalarType::Text),
    ("Flag", ScalarType::Int16),
    ("LvExtra", ScalarType::Int32),
    ("Name1", ScalarType::Text),
    ("Name2", ScalarType::Text),
    ("ObjectId", ScalarType::Int32),
    ("Order", ScalarType::Binary),
];

pub(super) struct Columns {
    pub positions: [usize; FIELD_COUNT],
}
pub(super) fn read(
    table: &TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Columns, InspectError> {
    if table.columns().len() != FIELD_COUNT {
        return Err(InspectError::Unsupported);
    }
    let mut positions = [0; FIELD_COUNT];
    for (i, (name, kind)) in FIELDS.iter().enumerate() {
        check(budget, cancelled)?;
        let (position, column) = table
            .columns()
            .iter()
            .enumerate()
            .find(|(_, column)| column.name().eq_ignore_ascii_case(name))
            .ok_or(InspectError::Unsupported)?;
        if column.data_type() != *kind {
            return Err(InspectError::Unsupported);
        }
        positions[i] = position;
    }
    Ok(Columns { positions })
}
