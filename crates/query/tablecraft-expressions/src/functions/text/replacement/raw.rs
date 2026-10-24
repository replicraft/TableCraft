//! Lossless unit replacement, with explicit provider spans and metered output.
use super::Options;
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch, StringSearchProfile},
    functions::text::output::ByteOutput,
};
use std::ops::Range;

impl Options {
    /// Keep the borrowed Text path unless the requested suffix splits a pair.
    pub(crate) fn text_suffix(
        &self,
        source: &str,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<bool, EvalError> {
        if self.start == 0 {
            return Ok(true);
        }
        let mut units = 0;
        for character in source.chars() {
            step()?;
            if units == self.start {
                return Ok(true);
            }
            units += character.len_utf16();
            if units > self.start {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn needs_search(&self, length: usize, find_length: usize) -> bool {
        self.start < length && find_length != 0 && self.count != 0
    }

    pub(crate) fn selected_mode(
        &self,
        collation: Option<&dyn Collation>,
    ) -> Result<StringCompareMode, EvalError> {
        self.mode
            .or_else(|| collation.and_then(Collation::option_compare))
            .ok_or(EvalError::CollationRequired)
    }

    /// Copy complete units without Unicode publication. A match must consume
    /// source units, so unlimited replacement always advances. Inserted units
    /// are never searched. The caller must discard private output on failure.
    pub(crate) fn write_utf16(
        self,
        source: &[u16],
        find: &[u16],
        replacement: &[u16],
        collation: Option<&dyn Collation>,
        output: &mut impl ByteOutput,
    ) -> Result<(), EvalError> {
        output.step()?;
        let mut remaining = &source[self.start.min(source.len())..];
        if !self.needs_search(source.len(), find.len()) {
            return append(remaining, output);
        }
        let mode = self.selected_mode(collation)?;
        let mut count = self.count;
        while !remaining.is_empty() && count != 0 {
            output.step()?;
            let found = if mode == StringCompareMode::Binary {
                binary(remaining, find, output)?
            } else {
                let provider = collation.ok_or(EvalError::CollationRequired)?;
                if provider.string_search_profile(mode) != StringSearchProfile::Utf16 {
                    return Err(EvalError::CollationRequired);
                }
                let found =
                    provider.find_utf16(remaining, find, StringSearch::ForwardFrom(0), mode)?;
                output.step()?;
                if let Some(found) = &found
                    && (found.start >= found.end || found.end > remaining.len())
                {
                    return Err(EvalError::InvalidTextMatch);
                }
                found
            };
            let Some(found) = found else {
                break;
            };
            append(&remaining[..found.start], output)?;
            append(replacement, output)?;
            remaining = &remaining[found.end..];
            if count > 0 {
                count -= 1;
            }
        }
        append(remaining, output)
    }
}

fn append(units: &[u16], output: &mut impl ByteOutput) -> Result<(), EvalError> {
    for unit in units {
        output.append(&unit.to_le_bytes())?;
    }
    Ok(())
}

fn binary(
    source: &[u16],
    find: &[u16],
    output: &mut impl ByteOutput,
) -> Result<Option<Range<usize>>, EvalError> {
    for (start, candidate) in source.windows(find.len()).enumerate() {
        output.step()?;
        let mut equal = true;
        for (left, right) in candidate.iter().zip(find) {
            output.step()?;
            if left != right {
                equal = false;
                break;
            }
        }
        if equal {
            return Ok(Some(start..start + find.len()));
        }
    }
    Ok(None)
}
