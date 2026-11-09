//! Sparse byte offsets for bounded, virtual text rows. Original text stays owned
//! by the current Data publication; display wrapping never changes Copy output.
use std::ops::Range;

pub(super) const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ROW_SCALARS: usize = 1024;
const CHECKPOINT_ROWS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IndexError {
    Limit,
    Allocation,
    Changed,
}

pub(super) struct Index {
    bytes: usize,
    checkpoints: Vec<u32>,
    rows: usize,
    pub continued_lines: bool,
}

struct Row {
    text: Range<usize>,
    next: usize,
    finished: bool,
    continued: bool,
}

impl Index {
    pub fn build(value: &str) -> Result<Self, IndexError> {
        if value.len() > MAX_SOURCE_BYTES {
            return Err(IndexError::Limit);
        }

        // A nonfinal row consumes at least one byte. This also covers a final
        // empty row after a line break, without an offset for every blank line.
        let capacity = value.len() / CHECKPOINT_ROWS + 2;
        let mut checkpoints = Vec::new();
        checkpoints
            .try_reserve_exact(capacity)
            .map_err(|_| IndexError::Allocation)?;
        let mut cursor = 0;
        let mut rows = 0;
        let mut continued_lines = false;
        loop {
            if rows % CHECKPOINT_ROWS == 0 {
                checkpoints.push(u32::try_from(cursor).map_err(|_| IndexError::Limit)?);
            }
            let row = next_row(value, cursor)?;
            rows += 1;
            continued_lines |= row.continued;
            if row.finished {
                break;
            }
            cursor = row.next;
        }

        Ok(Self {
            bytes: value.len(),
            checkpoints,
            rows,
            continued_lines,
        })
    }

    pub fn row_count(&self) -> usize {
        self.rows
    }

    pub fn visible(
        &self,
        value: &str,
        requested: Range<usize>,
    ) -> Result<Vec<Range<usize>>, IndexError> {
        if value.len() != self.bytes || requested.start > requested.end || requested.end > self.rows
        {
            return Err(IndexError::Changed);
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(requested.len())
            .map_err(|_| IndexError::Allocation)?;
        if requested.is_empty() {
            return Ok(result);
        }

        let checkpoint = requested.start / CHECKPOINT_ROWS;
        let mut cursor = *self
            .checkpoints
            .get(checkpoint)
            .ok_or(IndexError::Changed)? as usize;
        for row_number in checkpoint * CHECKPOINT_ROWS..requested.end {
            let row = next_row(value, cursor)?;
            if row_number >= requested.start {
                result.push(row.text);
            }
            cursor = row.next;
        }
        Ok(result)
    }
}

fn next_row(value: &str, start: usize) -> Result<Row, IndexError> {
    let suffix = value.get(start..).ok_or(IndexError::Changed)?;
    for (number, (offset, character)) in suffix.char_indices().enumerate() {
        let end = start + offset + character.len_utf8();
        if matches!(
            character,
            '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'
        ) {
            let crlf = character == '\r' && value.as_bytes().get(end) == Some(&b'\n');
            return Ok(Row {
                text: start..start + offset,
                next: end + usize::from(crlf),
                finished: false,
                continued: false,
            });
        }
        if number + 1 == MAX_ROW_SCALARS {
            let next_is_break = value[end..].chars().next().is_some_and(is_break);
            if next_is_break {
                continue;
            }
            return Ok(Row {
                text: start..end,
                next: end,
                finished: end == value.len(),
                continued: end < value.len(),
            });
        }
    }

    Ok(Row {
        text: start..value.len(),
        next: value.len(),
        finished: true,
        continued: false,
    })
}

fn is_break(character: char) -> bool {
    matches!(
        character,
        '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

#[derive(Default)]
pub(super) struct Visible {
    requested: Option<Range<usize>>,
    pub rows: Vec<Range<usize>>,
}

impl Visible {
    pub fn update(
        &mut self,
        index: &Index,
        value: &str,
        requested: Range<usize>,
    ) -> Result<(), IndexError> {
        if self.requested.as_ref() != Some(&requested) {
            // Range is a small non-Copy descriptor consumed by indexing/visible-row lookup
            // while the same original bounds remain cached or iterated; no underlying text is
            // copied.
            self.rows = index.visible(value, requested.clone())?;
            self.requested = Some(requested);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
