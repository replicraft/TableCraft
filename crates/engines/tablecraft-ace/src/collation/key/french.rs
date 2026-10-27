//! Exact French legacy secondary stream transformation, not SQL comparison.
use super::{
    buffer::{Secondary, Work},
    tail::PLACEHOLDER,
};
use crate::InspectError;

pub(super) fn reverse_secondary(
    secondary: &mut Secondary,
    characters: usize,
    work: &mut Work<'_>,
) -> Result<(), InspectError> {
    work.tick()?;
    for _ in secondary.characters..characters {
        work.tick()?;
    }
    secondary.pad(characters, PLACEHOLDER)?;

    let length = secondary.bytes.len();
    for left in 0..length / 2 {
        work.tick()?;
        secondary.bytes.bytes_mut().swap(left, length - left - 1);
    }
    work.tick()
}
