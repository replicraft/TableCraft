//! Bounded renderer-neutral text cells for read-only host views.
//! Native scalar representation remains in the immutable session; this is not
//! an edit carrier, native field-format evaluator, or lossless export protocol.
use super::{Cell, CellKind, Error, controls::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const OUTPUT_BYTES: u64 = 8 * 1024 * 1024;

pub(super) struct Presentation<'a> {
    work: ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> Presentation<'a> {
    pub(super) fn new(cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            work: ResourceBudget::new(OUTPUT_BYTES, 0, 100_000),
            cancelled,
        }
    }

    pub(super) fn work(&mut self, steps: u64) -> Result<(), Error> {
        check(self.cancelled)?;
        self.work.charge(Resource::Steps, steps)?;
        Ok(())
    }

    pub(super) fn columns(&mut self, count: usize) -> Result<Vec<String>, Error> {
        self.work(1)?;
        self.work
            .charge_allocation(count as u64, size_of::<String>() as u64)?;
        let mut columns = Vec::new();
        columns.try_reserve_exact(count)?;
        Ok(columns)
    }

    pub(super) fn rows(&mut self, count: usize) -> Result<Vec<Vec<Cell>>, Error> {
        self.work(1)?;
        self.work
            .charge_allocation(count as u64, size_of::<Vec<Cell>>() as u64)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)?;
        Ok(rows)
    }

    pub(super) fn row(&mut self, count: usize) -> Result<Vec<Cell>, Error> {
        self.work(1)?;
        self.work
            .charge_allocation(count as u64, size_of::<Cell>() as u64)?;
        let mut row = Vec::new();
        row.try_reserve_exact(count)?;
        Ok(row)
    }

    pub(super) fn flags(&mut self, count: usize) -> Result<Vec<bool>, Error> {
        self.work(1)?;
        self.work
            .charge_allocation(count as u64, size_of::<bool>() as u64)?;
        let mut flags = Vec::new();
        flags.try_reserve_exact(count)?;
        Ok(flags)
    }

    pub(super) fn scalar(
        &mut self,
        value: &Value,
        stored_calculation: bool,
    ) -> Result<Cell, Error> {
        Ok(Cell {
            text: self.value(value)?,
            kind: CellKind::Scalar {
                is_null: matches!(value, Value::Null),
                stored_calculation,
                boolean: match value {
                    Value::Boolean(value) => Some(*value),
                    _ => None,
                },
                display_control: crate::DisplayControl::Uninspected,
            },
        })
    }

    pub(super) fn text(&mut self, text: &str) -> Result<String, Error> {
        let mut owned = String::new();
        self.append_text(&mut owned, text)?;
        Ok(owned)
    }

    #[cfg(all(
        feature = "offline-edit",
        any(target_os = "macos", target_os = "linux")
    ))]
    pub(crate) fn binary(&mut self, count: usize) -> Result<Vec<u8>, Error> {
        self.work(1)?;
        self.work.charge_allocation(count as u64, 1)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(count)?;
        Ok(bytes)
    }

    pub(crate) fn append_text(&mut self, owned: &mut String, fragment: &str) -> Result<(), Error> {
        check(self.cancelled)?;
        self.work.charge(Resource::Steps, 1)?;
        self.work.charge(Resource::Bytes, fragment.len() as u64)?;
        owned.try_reserve(fragment.len())?;
        owned.push_str(fragment);
        Ok(())
    }

    pub(super) fn value(&mut self, value: &Value) -> Result<String, Error> {
        let text = match value {
            Value::Text(text) => return self.text(text),
            Value::Null => "NULL".to_owned(),
            Value::Boolean(value) => value.to_string(),
            Value::Byte(value) => value.to_string(),
            Value::Int16(value) => value.to_string(),
            Value::Int32(value) => value.to_string(),
            Value::Int64(value) => value.to_string(),
            Value::Single(value) => value.to_float().to_string(),
            Value::Double(value) => value.to_float().to_string(),
            Value::Currency(value) => value.to_string(),
            Value::Decimal(value) => value.to_string(),
            Value::Binary(bytes) => {
                check(self.cancelled)?;
                let length = bytes
                    .len()
                    .checked_mul(2)
                    .and_then(|value| value.checked_add(2))
                    .ok_or(Error::BinaryLimit)?;
                self.work.charge(Resource::Bytes, length as u64)?;
                let mut text = String::with_capacity(length);
                text.push_str("0x");
                for byte in bytes {
                    check(self.cancelled)?;
                    self.work.charge(Resource::Steps, 1)?;
                    use std::fmt::Write;
                    write!(text, "{byte:02x}").map_err(|_| Error::Formatting)?;
                }
                return Ok(text);
            }
            Value::Guid(value) => {
                let b = value.to_bytes();
                format!(
                    "{{{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
                    b[0],
                    b[1],
                    b[2],
                    b[3],
                    b[4],
                    b[5],
                    b[6],
                    b[7],
                    b[8],
                    b[9],
                    b[10],
                    b[11],
                    b[12],
                    b[13],
                    b[14],
                    b[15]
                )
            }
            Value::DateTime(value) => {
                let date = value.calendar_rounded_to_nanos()?;
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}",
                    date.year(),
                    date.month(),
                    date.day(),
                    date.hour(),
                    date.minute(),
                    date.second(),
                    date.subsec_nanosecond()
                )
            }
            Value::DateTimeExtended(value) => {
                let (year, month, day) = value.date();
                let (hour, minute, second, fraction) = value.time();
                format!(
                    "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{fraction:07}"
                )
            }
        };
        self.text(&text)
    }
}

#[cfg(test)]
mod tests;
