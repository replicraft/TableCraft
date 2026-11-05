//! Display positions and immutable native snapshot ordinals are separate domains.
use super::Data;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowOrdinals {
    values: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RowOrdinalError {
    #[error("native row ordinals must be strictly increasing and unique")]
    Order,
    #[error("native row ordinal count differs from the displayed rows")]
    Count,
    #[error("native row ordinals require a native source revision")]
    Source,
    #[error("native row ordinal exceeds the supported range")]
    Overflow,
}

impl RowOrdinals {
    pub fn new(values: Vec<u32>) -> Result<Self, RowOrdinalError> {
        if values.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(RowOrdinalError::Order);
        }
        Ok(Self { values })
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.values
    }
}

impl Data {
    /// Validate the page/map relationship without rescanning checked membership.
    /// A map's visible offset is independent of its physical native ordinals.
    pub fn validate_row_ordinals(&self) -> Result<(), RowOrdinalError> {
        if let Some(ordinals) = &self.row_ordinals {
            if self.source_revision.is_none() {
                return Err(RowOrdinalError::Source);
            }
            if ordinals.values.len() != self.rows.len() {
                return Err(RowOrdinalError::Count);
            }
        }
        if !self.rows.is_empty() {
            let last = u32::try_from(self.rows.len() - 1).map_err(|_| RowOrdinalError::Overflow)?;
            self.offset
                .checked_add(last)
                .ok_or(RowOrdinalError::Overflow)?;
        }
        Ok(())
    }

    /// Resolve a displayed row to its native snapshot ordinal. Never fall back
    /// to contiguous arithmetic when an explicit map is malformed.
    pub fn parent_ordinal(&self, row: usize) -> Option<u32> {
        self.source_revision?;
        self.validate_row_ordinals().ok()?;
        self.rows.get(row)?;
        match &self.row_ordinals {
            Some(ordinals) => ordinals.values.get(row).copied(),
            None => self.offset.checked_add(u32::try_from(row).ok()?),
        }
    }

    /// Reverse lookup is checked against this exact displayed page only.
    pub fn row_index(&self, parent_ordinal: u32) -> Option<usize> {
        self.source_revision?;
        self.validate_row_ordinals().ok()?;
        let row = match &self.row_ordinals {
            Some(ordinals) => ordinals.values.binary_search(&parent_ordinal).ok()?,
            None => usize::try_from(parent_ordinal.checked_sub(self.offset)?).ok()?,
        };
        self.rows.get(row)?;
        Some(row)
    }

    /// Zero-based visible position; this is not a native write or collection ID.
    pub fn display_ordinal(&self, row: usize) -> Option<u32> {
        self.validate_row_ordinals().ok()?;
        self.rows.get(row)?;
        self.offset.checked_add(u32::try_from(row).ok()?)
    }
}

#[cfg(test)]
mod tests;
