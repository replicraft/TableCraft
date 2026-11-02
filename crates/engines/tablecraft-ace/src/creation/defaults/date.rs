//! Explicit civil declaration syntax, admitted only after exact DATE bit replay.
use crate::{CreateDatabaseError, InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::LegacyDateTime,
};

const MAX_LITERAL_BYTES: usize = 31;
const CALENDAR_WORK: u64 = 64;
const DECIMAL_RADIX: u32 = 10;
const FRACTION_DIGITS: usize = 9;
const YEAR_DIGITS: usize = 4;
const COMPONENT_DIGITS: usize = 2;

pub(super) struct Literal {
    bytes: [u8; MAX_LITERAL_BYTES],
    length: usize,
}

impl Literal {
    fn byte(
        &mut self,
        value: u8,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        check(budget, cancelled)?;
        let destination = self
            .bytes
            .get_mut(self.length)
            .ok_or(CreateDatabaseError::Declaration)?;
        *destination = value;
        self.length += 1;
        Ok(())
    }

    fn digits(
        &mut self,
        mut value: u32,
        width: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        let mut buffer = [b'0'; FRACTION_DIGITS];
        let start = buffer
            .len()
            .checked_sub(width)
            .ok_or(CreateDatabaseError::Declaration)?;
        for digit in buffer[start..].iter_mut().rev() {
            check(budget, cancelled)?;
            *digit += (value % DECIMAL_RADIX) as u8;
            value /= DECIMAL_RADIX;
        }
        if value != 0 {
            return Err(CreateDatabaseError::Declaration);
        }
        for digit in &buffer[start..] {
            self.byte(*digit, budget, cancelled)?;
        }
        Ok(())
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

pub(super) fn capture(
    value: LegacyDateTime,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Literal, CreateDatabaseError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, CALENDAR_WORK)
        .map_err(InspectError::from)?;
    let calendar = value
        .calendar_rounded_to_nanos()
        .map_err(|_| CreateDatabaseError::Declaration)?;
    let reencoded =
        LegacyDateTime::from_calendar(calendar).map_err(|_| CreateDatabaseError::Declaration)?;
    if reencoded.to_bits() != value.to_bits() {
        return Err(CreateDatabaseError::Declaration);
    }
    check(budget, cancelled)?;
    let mut result = Literal {
        bytes: [0; MAX_LITERAL_BYTES],
        length: 0,
    };
    result.byte(b'#', budget, cancelled)?;
    result.digits(calendar.year() as u32, YEAR_DIGITS, budget, cancelled)?;
    result.byte(b'-', budget, cancelled)?;
    result.digits(calendar.month() as u32, COMPONENT_DIGITS, budget, cancelled)?;
    result.byte(b'-', budget, cancelled)?;
    result.digits(calendar.day() as u32, COMPONENT_DIGITS, budget, cancelled)?;
    result.byte(b' ', budget, cancelled)?;
    result.digits(calendar.hour() as u32, COMPONENT_DIGITS, budget, cancelled)?;
    result.byte(b':', budget, cancelled)?;
    result.digits(
        calendar.minute() as u32,
        COMPONENT_DIGITS,
        budget,
        cancelled,
    )?;
    result.byte(b':', budget, cancelled)?;
    result.digits(
        calendar.second() as u32,
        COMPONENT_DIGITS,
        budget,
        cancelled,
    )?;
    let mut fraction = calendar.subsec_nanosecond() as u32;
    if fraction != 0 {
        let mut width = FRACTION_DIGITS;
        while fraction.is_multiple_of(DECIMAL_RADIX) {
            check(budget, cancelled)?;
            fraction /= DECIMAL_RADIX;
            width -= 1;
        }
        result.byte(b'.', budget, cancelled)?;
        result.digits(fraction, width, budget, cancelled)?;
    }
    result.byte(b'#', budget, cancelled)?;
    let contents = std::str::from_utf8(&result.bytes()[1..result.length - 1])
        .map_err(|_| CreateDatabaseError::Declaration)?;
    budget
        .charge(Resource::Steps, (2 * contents.len()) as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    let parsed = tablecraft_expressions::datetime::parse_query_literal(contents)
        .map_err(|_| CreateDatabaseError::Declaration)?;
    if parsed.to_bits() != value.to_bits() {
        return Err(CreateDatabaseError::Declaration);
    }
    check(budget, cancelled)?;
    Ok(result)
}
