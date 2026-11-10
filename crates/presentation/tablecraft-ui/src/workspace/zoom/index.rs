//! Sparse byte offsets for bounded, virtual text rows. Original text stays owned
//! by the current Data publication; display wrapping never changes Copy output.
use std::ops::Range;

const MAX_ROW_SCALARS: usize = 1024;
const CHECKPOINT_ROWS: usize = 32;
const INDEX_BYTES_PER_FRAME: usize = 64 * 1024;
const INDEX_ROWS_PER_FRAME: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IndexError {
    Allocation,
    Changed,
}

pub(super) struct Index {
    bytes: usize,
    checkpoints: Vec<usize>,
    cursor: usize,
    finished: bool,
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
    pub fn new(value: &str) -> Self {
        Self {
            bytes: value.len(),
            checkpoints: Vec::new(),
            cursor: 0,
            finished: false,
            rows: 0,
            continued_lines: false,
        }
    }

    /// Advance only one frame's row/byte work. A row can extend the byte budget
    /// by at most its bounded UTF-8 visual segment; source bytes are never copied.
    pub fn advance(&mut self, value: &str) -> Result<(), IndexError> {
        if value.len() != self.bytes {
            return Err(IndexError::Changed);
        }
        let initial_cursor = self.cursor;
        let initial_rows = self.rows;
        while !self.finished
            && self.cursor - initial_cursor < INDEX_BYTES_PER_FRAME
            && self.rows - initial_rows < INDEX_ROWS_PER_FRAME
        {
            if self.rows.is_multiple_of(CHECKPOINT_ROWS) {
                self.checkpoints
                    .try_reserve(1)
                    .map_err(|_| IndexError::Allocation)?;
                self.checkpoints.push(self.cursor);
            }
            let row = next_row(value, self.cursor)?;
            self.rows = self.rows.checked_add(1).ok_or(IndexError::Changed)?;
            self.continued_lines |= row.continued;
            self.cursor = row.next;
            self.finished = row.finished;
        }
        Ok(())
    }

    pub fn complete(&self) -> bool {
        self.finished
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
            .ok_or(IndexError::Changed)?;
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
            // NO_CLONE: Range is a small non-Copy descriptor consumed by indexing/visible-row
            // lookup while the same original bounds remain cached or iterated; no
            // underlying text is copied.
            self.rows = index.visible(value, requested.clone())?;
            self.requested = Some(requested);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
