//! Bounded transport only; existing UPDATE converters own all value semantics.
use super::{MAX_REQUESTS, reader, reserve};
use crate::commands::write::{
    error::{Error, input},
    tick, updates,
};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::commands::write) fn cells(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<String>, Error> {
    let (mut reader, mut record) = reader(bytes, budget, cancelled)?;
    let mut cells = Vec::new();
    let mut rows = 0;
    while reader
        .read_byte_record(&mut record)
        .map_err(|_| input("Malformed quoted UPDATE TSV"))?
    {
        tick(budget, cancelled)?;
        if rows == MAX_REQUESTS || record.len() != updates::SET_FIELDS {
            return Err(input("Expected 1..4096 six-column UPDATE requests"));
        }
        reserve(&mut cells, updates::SET_FIELDS, budget)?;
        for field in &record {
            tick(budget, cancelled)?;
            let text =
                std::str::from_utf8(field).map_err(|_| input("UPDATE field is not UTF-8"))?;
            budget
                .charge_allocation(text.len() as u64, 1)
                .map_err(InspectError::from)?;
            budget
                .charge(Resource::Steps, text.len() as u64)
                .map_err(InspectError::from)?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| InspectError::Allocation)?;
            owned.push_str(text);
            cells.push(owned);
        }
        rows += 1;
    }
    if rows == 0 {
        return Err(input("UPDATE request file contains no rows"));
    }
    tick(budget, cancelled)?;
    Ok(cells)
}
