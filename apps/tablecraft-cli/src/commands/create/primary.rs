//! Explicit CLI selectors and paid adaptation; native constraints stay in ACE.
use super::{Error, input::reserve, tick};
use tablecraft_access::creation::CreationError;
use tablecraft_ace::{CreateColumn, InspectError};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const SELECTOR_WORDS: usize = 2;

pub(super) fn bind<'a>(
    columns: &mut [CreateColumn<'a>],
    selection: &'a [String],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let [name, index] = selection else {
        return Err(Error::Input(
            "Expected --primary-key COLUMN INDEX_NAME exactly once",
        ));
    };
    debug_assert_eq!(selection.len(), SELECTOR_WORDS);
    let mut selected = None;
    for (position, column) in columns.iter().enumerate() {
        tick(budget, cancelled)?;
        budget
            .charge(Resource::Steps, column.name().len().max(name.len()) as u64)
            .map_err(InspectError::from)?;
        if column.name().eq_ignore_ascii_case(name) && selected.replace(position).is_some() {
            return Err(Error::Input(
                "Primary-key column selector matches more than one declaration",
            ));
        }
    }
    let selected = selected.ok_or(Error::Input(
        "Primary-key column selector does not match a declared column",
    ))?;
    columns[selected] = columns[selected]
        .with_primary_key(index)
        .map_err(|cause| Error::Creation(CreationError::Create(cause)))?;
    Ok(())
}

pub(super) fn values(
    rows: &[Vec<i32>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Value>>, Error> {
    let mut result = Vec::new();
    reserve(&mut result, rows.len(), budget)?;
    for original in rows {
        tick(budget, cancelled)?;
        let mut row = Vec::new();
        reserve(&mut row, original.len(), budget)?;
        for value in original {
            tick(budget, cancelled)?;
            row.push(Value::Int32(*value));
        }
        result.push(row);
    }
    Ok(result)
}
