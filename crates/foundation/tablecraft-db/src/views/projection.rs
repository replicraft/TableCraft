//! Borrowed projected rows without cloning cells or padding missing values.
use tablecraft_core::values::Value;
use thiserror::Error;

/// Borrowed dense/projected source values. Construction performs no coercion;
/// the executor checks the requested width and exact carriers before copying.
#[derive(Clone, Copy)]
pub struct ProjectedRow<'a> {
    values: &'a [Value],
    indices: Option<&'a [usize]>,
}

impl<'a> ProjectedRow<'a> {
    /// Borrow an already densely projected row.
    pub const fn dense(values: &'a [Value]) -> Self {
        Self {
            values,
            indices: None,
        }
    }

    /// Borrow selected cells of an existing row, requiring sorted unique positions.
    /// Empty selections retain the source record as a zero-width row.
    pub fn select(values: &'a [Value], indices: &'a [usize]) -> Result<Self, ProjectionError> {
        validate_projection(indices, values.len())?;

        Ok(Self {
            values,
            indices: Some(indices),
        })
    }

    /// Number of exposed cells, excluding unrequested cells.
    pub fn len(self) -> usize {
        self.indices.map_or(self.values.len(), <[usize]>::len)
    }

    /// Whether this source record exposes no value cells.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Borrow one requested cell; malformed/missing positions return None.
    pub fn get(self, index: usize) -> Option<&'a Value> {
        self.values.get(match self.indices {
            Some(indices) => *indices.get(index)?,
            None => index,
        })
    }
}

/// Validate sorted unique positions against a complete scalar source width.
/// Empty projections are valid; invalid requests return a typed source error.
pub fn validate_projection(indices: &[usize], width: usize) -> Result<(), ProjectionError> {
    if let Some(&index) = indices.last()
        && index >= width
    {
        return Err(ProjectionError::OutOfBounds { index, width });
    }

    if indices.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ProjectionError::Unordered);
    }

    Ok(())
}

/// Invalid positions in a borrowed row projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ProjectionError {
    /// At least one requested position lies outside the complete row.
    #[error("row projection index {index} exceeds width {width}")]
    OutOfBounds {
        /// Final requested position.
        index: usize,
        /// Complete source row width.
        width: usize,
    },
    /// Positions must be strictly increasing, without duplicates.
    #[error("row projection positions are not strictly increasing")]
    Unordered,
}
