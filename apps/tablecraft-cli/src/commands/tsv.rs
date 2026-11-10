//! Shared strict TSV quoting before the deliberately permissive csv decoder.
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const DELIMITER: u8 = b'\t';
const QUOTE: u8 = b'"';
const CANCEL_INTERVAL: usize = 4096;

#[derive(Debug)]
pub(crate) enum Error {
    Native(InspectError),
    Malformed(&'static str),
}

impl From<InspectError> for Error {
    fn from(error: InspectError) -> Self {
        Self::Native(error)
    }
}

fn input(message: &'static str) -> Error {
    Error::Malformed(message)
}

fn tick(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum State {
    Start,
    Unquoted,
    Quoted,
    Closed,
}

/// Encoded record maxima are conservative decoder capacity bounds. Quoted
/// delimiters and newlines remain data, never extra fields or records.
pub(crate) struct RecordLayout {
    pub max_record_bytes: usize,
    pub max_fields: usize,
}

impl RecordLayout {
    fn include(&mut self, bytes: usize, fields: usize) {
        self.max_record_bytes = self.max_record_bytes.max(bytes);
        self.max_fields = self.max_fields.max(fields);
    }
}

pub(crate) fn validate(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecordLayout, Error> {
    tick(budget, cancelled)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let mut state = State::Start;
    let mut position = 0;
    let mut next_poll = 0;
    let mut record_start = 0;
    let mut fields = 1_usize;
    let mut layout = RecordLayout {
        max_record_bytes: 0,
        max_fields: 0,
    };
    while position < bytes.len() {
        if position >= next_poll {
            tick(budget, cancelled)?;
            next_poll = position.saturating_add(CANCEL_INTERVAL);
        }
        let byte = bytes[position];
        match state {
            State::Quoted => {
                if byte == QUOTE {
                    if bytes.get(position + 1) == Some(&QUOTE) {
                        position += 1;
                    } else {
                        state = State::Closed;
                    }
                }
            }
            State::Start | State::Unquoted | State::Closed => {
                if byte == b'\r' {
                    if bytes.get(position + 1) != Some(&b'\n') {
                        return Err(input("Bare CR outside a quoted TSV field"));
                    }
                    position += 1;
                    layout.include(position + 1 - record_start, fields);
                    record_start = position + 1;
                    fields = 1;
                    state = State::Start;
                } else if byte == b'\n' {
                    layout.include(position + 1 - record_start, fields);
                    record_start = position + 1;
                    fields = 1;
                    state = State::Start;
                } else if byte == DELIMITER {
                    fields = fields.checked_add(1).ok_or(InspectError::StructureLimit)?;
                    state = State::Start;
                } else if matches!(state, State::Closed) {
                    return Err(input("Unexpected bytes after a closing TSV quote"));
                } else if byte == QUOTE {
                    if !matches!(state, State::Start) {
                        return Err(input("A TSV quote must begin a field"));
                    }
                    state = State::Quoted;
                } else {
                    state = State::Unquoted;
                }
            }
        }
        position += 1;
    }
    if matches!(state, State::Quoted) {
        return Err(input("Unterminated quoted TSV field"));
    }
    if record_start < bytes.len() {
        layout.include(bytes.len() - record_start, fields);
    }
    tick(budget, cancelled)?;
    Ok(layout)
}
