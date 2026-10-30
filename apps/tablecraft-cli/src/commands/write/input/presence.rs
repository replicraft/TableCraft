//! Parse original field presence without consulting metadata or expanding values.
use super::{Error, input, reserve, tick, values};
use tablecraft_ace::{InsertFieldValue, OmittedRowInsert};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::commands::write) enum Parsed<T> {
    Complete(Vec<T>),
    Omitted(Vec<OmittedRowInsert>),
}

pub(in crate::commands::write) fn inserts<T>(
    bytes: &[u8],
    max_value_bytes: usize,
    make_complete: fn(Vec<Value>) -> T,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Parsed<T>, Error> {
    let rows: Vec<Vec<InsertFieldValue>> = super::insert_fields(
        bytes,
        max_value_bytes,
        field,
        std::convert::identity,
        budget,
        cancelled,
    )?;
    let mut omitted = false;
    for row in &rows {
        for value in row {
            tick(budget, cancelled)?;
            omitted |= matches!(value, InsertFieldValue::Omitted);
        }
    }
    if omitted {
        let mut requests = Vec::new();
        reserve(&mut requests, rows.len(), budget)?;
        for row in rows {
            tick(budget, cancelled)?;
            requests.push(OmittedRowInsert::new(row));
        }
        return Ok(Parsed::Omitted(requests));
    }

    // Complete rows keep the old native API and its passive default metadata
    // admission. Move values; neither NULL nor absent clock becomes a default.
    let mut complete = Vec::new();
    reserve(&mut complete, rows.len(), budget)?;
    for row in rows {
        let mut supplied = Vec::new();
        reserve(&mut supplied, row.len(), budget)?;
        for value in row {
            tick(budget, cancelled)?;
            let InsertFieldValue::Supplied(value) = value else {
                return Err(input("Complete row contains an omitted field"));
            };
            supplied.push(value);
        }
        complete.push(make_complete(supplied));
    }
    tick(budget, cancelled)?;
    Ok(Parsed::Complete(complete))
}

fn field(kind: &str, data: &str, budget: &mut ResourceBudget) -> Result<InsertFieldValue, Error> {
    if kind == "omitted" {
        budget
            .charge(Resource::Steps, (kind.len() + data.len()) as u64)
            .map_err(tablecraft_ace::InspectError::from)?;
        if !data.is_empty() {
            return Err(input("omitted KIND requires empty DATA"));
        }
        return Ok(InsertFieldValue::Omitted);
    }
    values::value(kind, data, false, budget).map(InsertFieldValue::Supplied)
}
