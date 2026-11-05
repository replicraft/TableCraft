//! Display positions and immutable native snapshot ordinals are separate domains.
use super::Data;

const MAX_ROW_IDENTITY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowOrdinals {
    values: Vec<u32>,
    inverse: Option<Vec<(u32, usize)>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RowOrdinalError {
    #[error("native row ordinals must be strictly increasing and unique")]
    Order,
    #[error("native row ordinals must be unique")]
    Duplicate,
    #[error("row identity storage exceeds the UI construction limit")]
    Limit,
    #[error("row identity lookup allocation failed")]
    Allocation,
    #[error("native row ordinal count differs from the displayed rows")]
    Count,
    #[error("native row ordinals require a native source revision")]
    Source,
    #[error("native row ordinal exceeds the supported range")]
    Overflow,
}

impl RowOrdinals {
    /// Source-ordered membership, as returned by ordinary filtered pages.
    pub fn new(values: Vec<u32>) -> Result<Self, RowOrdinalError> {
        check_storage(values.len(), false)?;
        if values.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(RowOrdinalError::Order);
        }
        Ok(Self {
            values,
            inverse: None,
        })
    }

    /// Display order issued by a native sorting service. This validates mapping
    /// structure only; the host must still validate its source and sort witness.
    pub fn from_display_order(values: Vec<u32>) -> Result<Self, RowOrdinalError> {
        check_storage(values.len(), false)?;
        let source_ordered = values.windows(2).all(|pair| pair[0] < pair[1]);
        if source_ordered {
            return Self::new(values);
        }
        check_storage(values.len(), true)?;
        let mut inverse = Vec::new();
        inverse
            .try_reserve_exact(values.len())
            .map_err(|_| RowOrdinalError::Allocation)?;
        for (row, &ordinal) in values.iter().enumerate() {
            inverse.push((ordinal, row));
        }
        inverse.sort_unstable_by_key(|entry| entry.0);
        if inverse.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(RowOrdinalError::Duplicate);
        }
        Ok(Self {
            values,
            inverse: Some(inverse),
        })
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.values
    }

    pub fn is_source_ordered(&self) -> bool {
        self.inverse.is_none()
    }

    fn row_index(&self, parent_ordinal: u32) -> Option<usize> {
        match &self.inverse {
            Some(inverse) => {
                let position = inverse
                    .binary_search_by_key(&parent_ordinal, |entry| entry.0)
                    .ok()?;
                Some(inverse[position].1)
            }
            None => self.values.binary_search(&parent_ordinal).ok(),
        }
    }
}

fn check_storage(length: usize, inverse: bool) -> Result<(), RowOrdinalError> {
    let entry_bytes = if inverse {
        size_of::<u32>() + size_of::<(u32, usize)>()
    } else {
        size_of::<u32>()
    };
    let bytes = length
        .checked_mul(entry_bytes)
        .ok_or(RowOrdinalError::Limit)?;
    if bytes > MAX_ROW_IDENTITY_BYTES {
        return Err(RowOrdinalError::Limit);
    }
    Ok(())
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
            Some(ordinals) => ordinals.row_index(parent_ordinal)?,
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
