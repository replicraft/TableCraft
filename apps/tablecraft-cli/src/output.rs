//! Human-readable buffered table; formatting policy stays at the CLI boundary.
use std::fmt::{self, Write};
use tablecraft_core::values::Value;

const MAX_OUTPUT_BYTES: usize = 1_048_576;
pub(crate) enum OutputError {
    Limit,
    Allocation,
    UnsupportedValue,
    Formatting,
}
impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Limit => "evaluation output limit exceeded",
            Self::Allocation => "evaluation output allocation failed",
            Self::UnsupportedValue => "evaluation output value is not supported",
            Self::Formatting => "evaluation output formatting failed",
        })
    }
}
pub(super) struct TableOutput {
    text: String,
    error: Option<OutputError>,
}
impl TableOutput {
    pub(super) fn new() -> Self {
        Self {
            text: String::new(),
            error: None,
        }
    }
    pub(super) fn finish(self) -> String {
        self.text
    }
    pub(super) fn separator(&mut self) -> Result<(), OutputError> {
        let result = self.write_char('\t');
        self.checked(result)
    }
    pub(super) fn newline(&mut self) -> Result<(), OutputError> {
        let result = self.write_char('\n');
        self.checked(result)
    }
    pub(super) fn header(&mut self, alias: Option<&str>, column: usize) -> Result<(), OutputError> {
        if let Some(alias) = alias {
            self.escaped(alias)
        } else {
            let result = write!(self, "Expr{column}");
            self.checked(result)
        }
    }
    pub(super) fn value(&mut self, value: &Value) -> Result<(), OutputError> {
        let result = match value {
            Value::Null => self.write_str("NULL"),
            Value::Boolean(value) => self.write_str(if *value { "TRUE" } else { "FALSE" }),
            Value::Byte(value) => write!(self, "{value}"),
            Value::Int16(value) => write!(self, "{value}"),
            Value::Int32(value) => write!(self, "{value}"),
            Value::Int64(value) => write!(self, "{value}"),
            Value::Single(value) => write!(self, "{}", value.to_float()),
            Value::Double(value) => write!(self, "{}", value.to_float()),
            Value::Currency(value) => write!(self, "{value}"),
            Value::Decimal(value) => write!(self, "{value}"),
            Value::DateTime(value) => {
                let dt = value
                    .calendar_rounded_to_nanos()
                    .map_err(|_| OutputError::UnsupportedValue)?;
                write!(
                    self,
                    "#{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}#",
                    dt.year(),
                    dt.month(),
                    dt.day(),
                    dt.hour(),
                    dt.minute(),
                    dt.second(),
                    dt.subsec_nanosecond()
                )
            }
            Value::DateTimeExtended(value) => {
                let (year, month, day) = value.date();
                let (hour, minute, second, fraction) = value.time();
                write!(
                    self,
                    "#{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{fraction:07}#"
                )
            }
            Value::Guid(value) => write!(
                self,
                "{{{}}}",
                uuid::Uuid::from_bytes(value.to_bytes()).hyphenated()
            ),
            Value::Binary(bytes) => {
                let result = self.write_str("0x");
                self.checked(result)?;
                for byte in bytes {
                    let result = write!(self, "{byte:02x}");
                    self.checked(result)?;
                }
                Ok(())
            }
            Value::Text(value) => {
                let result = self.write_char('"');
                self.checked(result)?;
                self.escaped(value)?;
                self.write_char('"')
            }
        };
        self.checked(result)
    }
    fn escaped(&mut self, text: &str) -> Result<(), OutputError> {
        for ch in text.chars() {
            let result = match ch {
                '\\' => self.write_str("\\\\"),
                '"' => self.write_str("\\\""),
                '\t' => self.write_str("\\t"),
                '\r' => self.write_str("\\r"),
                '\n' => self.write_str("\\n"),
                ch if ch.is_control() => write!(self, "\\u{{{:04x}}}", u32::from(ch)),
                ch => self.write_char(ch),
            };
            self.checked(result)?;
        }
        Ok(())
    }
    fn checked(&mut self, result: fmt::Result) -> Result<(), OutputError> {
        result.map_err(|_| match self.error.take() {
            Some(error) => error,
            None => OutputError::Formatting,
        })
    }
}
impl Write for TableOutput {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self
            .text
            .len()
            .checked_add(text.len())
            .is_none_or(|size| size > MAX_OUTPUT_BYTES)
        {
            self.error = Some(OutputError::Limit);
            return Err(fmt::Error);
        }
        if self.text.try_reserve(text.len()).is_err() {
            self.error = Some(OutputError::Allocation);
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
