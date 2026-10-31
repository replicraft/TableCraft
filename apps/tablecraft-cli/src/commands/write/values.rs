//! Existing engineering protocols map the shared exact parser into write errors.
use super::error::{Error, input};
use crate::commands::typed_values;
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(super) fn value(
    kind: &str,
    data: &str,
    null_word: bool,
    budget: &mut ResourceBudget,
) -> Result<Value, Error> {
    typed_values::value(kind, data, null_word, budget).map_err(error)
}

pub(super) fn update(kind: &str, data: &str, budget: &mut ResourceBudget) -> Result<Value, Error> {
    typed_values::update(kind, data, budget).map_err(error)
}

pub(super) fn fixed_kind(kind: &str) -> bool {
    typed_values::fixed_kind(kind)
}

pub(super) fn hex(data: &str, budget: &mut ResourceBudget) -> Result<Vec<u8>, Error> {
    typed_values::hex(data, budget).map_err(error)
}

fn error(cause: typed_values::Error) -> Error {
    match cause {
        typed_values::Error::Input(message) => input(message),
        typed_values::Error::Native(cause) => Error::Native(cause),
    }
}
