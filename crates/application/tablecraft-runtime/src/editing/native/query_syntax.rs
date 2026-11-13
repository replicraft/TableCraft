//! Shared paid syntax capture for real saved-definition consumers.
use crate::{Error, controls::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{ParseLimits, ParsedQuery, parse_query, select::Expression};

const SYNTAX_BYTES: u64 = 16 * 1024 * 1024;
const SYNTAX_METADATA_ROWS: u64 = 100_000;
const SYNTAX_STEPS: u64 = 1_000_000;
const PARSED_TEXT_COPIES: u64 = 2;

pub(super) fn syntax_budget() -> ResourceBudget {
    // Metadata reconstruction has its own cumulative counters. These are not
    // query execution allowances or a counter reset for each native fragment.
    ResourceBudget::new(SYNTAX_BYTES, SYNTAX_METADATA_ROWS, SYNTAX_STEPS)
}

pub(super) fn parsed(
    value: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ParsedQuery, Error> {
    check(cancelled)?;
    let limits = ParseLimits::default();
    if value.len() > limits.max_bytes {
        return Err(Error::SqlLimit);
    }
    let tokens = limits.max_tokens.min(value.len());
    work.charge(Resource::Steps, value.len() as u64 + tokens as u64)?;
    work.charge_allocation(tokens as u64, size_of::<Expression>() as u64)?;
    work.charge_allocation(value.len() as u64, PARSED_TEXT_COPIES)?;
    let parsed = parse_query(value, limits)?;
    check(cancelled)?;
    Ok(parsed)
}
