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

pub(crate) fn validate(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    tick(budget, cancelled)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let mut state = State::Start;
    let mut position = 0;
    let mut next_poll = 0;
    while position < bytes.len() {
        if position >= next_poll {
            tick(budget, cancelled)?;
            next_poll = position + CANCEL_INTERVAL;
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
                    state = State::Start;
                } else if matches!(byte, DELIMITER | b'\n') {
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
    tick(budget, cancelled)?;
    Ok(())
}
