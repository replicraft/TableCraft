//! Completed read-only evidence; no values, write authority or implicit Debug.
use crate::TableSchema;

/// Qualification of one ordinary native long-value column/map declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LongValueCheckStatus {
    /// Complete qualified descriptor, allocation and owned-page framing checks.
    Checked,
    /// No usable map pair was declared for this visible long-value column.
    Absent,
    /// The trailing definition could not be qualified as the observed list.
    UnknownDeclaration,
    /// A declaration names a column absent from the current visible schema.
    HistoricalDeclaration,
    /// Field flags/type do not match the ordinary qualified Memo/OLE profile.
    UnsupportedColumn,
    /// Existing indirect column maps have not received original-file qualification.
    IndirectMaps,
    /// A column map uses an unsupported representation or reference profile.
    UnknownMap,
    /// The exact ordinary ACE 16 format/subversion profile is not qualified.
    UnsupportedFormat,
}

/// Original packed column map references; inspection does not grant mutation.
#[derive(Clone, Copy)]
pub struct LongValueMapReferences {
    pub(super) owned: u32,
    pub(super) candidate: u32,
}

impl LongValueMapReferences {
    /// Original packed owned-map page/slot reference.
    pub const fn owned(self) -> u32 {
        self.owned
    }

    /// Original packed candidate-hint page/slot reference.
    pub const fn candidate(self) -> u32 {
        self.candidate
    }
}

/// Storage-kind counts over the column's completely visited live table roots.
#[derive(Clone, Copy, Default)]
pub struct LongValueCounts {
    pub(super) nulls: usize,
    pub(super) inline: usize,
    pub(super) single: usize,
    pub(super) chained: usize,
}

impl LongValueCounts {
    /// NULL/absent definitions, including fields absent from historical rows.
    pub const fn nulls(self) -> usize {
        self.nulls
    }

    /// Checked inline definitions, including non-NULL empty values.
    pub const fn inline(self) -> usize {
        self.inline
    }

    /// Checked single external-row definitions.
    pub const fn single(self) -> usize {
        self.single
    }

    /// Checked chained-kind definitions, including a single terminating chunk.
    pub const fn chained(self) -> usize {
        self.chained
    }
}

/// Explicit storage coverage for a native column number or historical declaration.
pub struct LongValueCheck {
    pub(super) number: usize,
    pub(super) maps: Option<LongValueMapReferences>,
    pub(super) status: LongValueCheckStatus,
    pub(super) counts: LongValueCounts,
    pub(super) owned_pages: usize,
    pub(super) candidate_pages: usize,
    pub(super) owned_rows: usize,
    pub(super) distinct_chunks: usize,
    pub(super) shared_references: usize,
    pub(super) shared_owned_pages: usize,
}

impl LongValueCheck {
    /// Native logical column number, independent of visible/display position.
    pub const fn column_number(&self) -> usize {
        self.number
    }

    /// Native declaration when present; unknown profiles remain visible.
    pub const fn maps(&self) -> Option<LongValueMapReferences> {
        self.maps
    }

    /// Qualification status, separate from ordinary value-read support.
    pub const fn status(&self) -> LongValueCheckStatus {
        self.status
    }

    /// Kind counts populated only for a completely checked column.
    pub const fn values(&self) -> LongValueCounts {
        self.counts
    }

    /// Framed pages in the declared column-owned set, including orphan storage.
    pub const fn owned_pages(&self) -> usize {
        self.owned_pages
    }

    /// Qualified candidate-hint population; false does not mean physically full.
    pub const fn candidate_pages(&self) -> usize {
        self.candidate_pages
    }

    /// Framed physical rows in owned pages, without interpreting orphan bodies.
    pub const fn owned_rows(&self) -> usize {
        self.owned_rows
    }

    /// Distinct physical chunks reached by the checked live values.
    pub const fn distinct_chunks(&self) -> usize {
        self.distinct_chunks
    }

    /// Additional live references to already visited chunks; not declared corrupt.
    pub const fn shared_references(&self) -> usize {
        self.shared_references
    }

    /// Owned rows not reached by any current checked descriptor; valid orphans.
    pub const fn orphan_rows(&self) -> usize {
        self.owned_rows - self.distinct_chunks
    }

    /// Owned pages also declared by another checked column; no exclusivity claim.
    pub const fn shared_owned_pages(&self) -> usize {
        self.shared_owned_pages
    }
}

/// Complete table-root visit and explicit per-column long-storage coverage.
/// Does not qualify indexes, constraints, macros, writes or concurrent-file isolation.
pub struct TableLongValueCheck {
    pub(super) schema: TableSchema,
    pub(super) rows: usize,
    pub(super) columns: Vec<LongValueCheck>,
}

impl TableLongValueCheck {
    /// Native metadata; historical declarations can lack a matching visible column.
    pub fn schema(&self) -> &TableSchema {
        &self.schema
    }

    /// All visited live logical roots, matched to the stored table count.
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Visible Memo/OLE columns plus retained unknown/historical declarations.
    pub fn columns(&self) -> &[LongValueCheck] {
        &self.columns
    }
}
