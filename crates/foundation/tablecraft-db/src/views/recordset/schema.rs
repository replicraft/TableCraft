//! Exact stored member schemas carry inspection availability independently of NULL.
use super::{RecordsetError, check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::{Column, NameComparison, validate_columns_with};

/// Whether the source inspected a member in this complete child-recordset view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberAvailability {
    /// Every child supplies an exact scalar or a genuinely inspected NULL.
    Available,
    /// No child value was inspected; declarations alone do not establish NULL.
    Uninspected,
}

/// Source-declared child-record semantics, without mutation or native-storage authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordsetKind {
    /// Generic read-only rows with no attachment or stored-history interpretation.
    Rows,
    /// Attachment metadata; payload availability remains explicit per member.
    Attachments,
    /// Stored versions in source order, separate from a visible current value.
    VersionHistory,
}

/// Validated immutable member order and availability, available before child scans.
/// It owns no native identity, source callback or mutation permission.
pub struct RecordsetSchema {
    columns: Vec<Column>,
    availability: Vec<MemberAvailability>,
    names: NameComparison,
    kind: RecordsetKind,
    retained_bytes: usize,
}

impl RecordsetSchema {
    /// Validate generic read-only rows under an explicit name comparison policy.
    /// The producer pays for input allocation; validation allocates nothing.
    ///
    /// # Errors
    /// Rejects empty schemas, duplicate names, unequal declaration/availability
    /// widths, exhausted work or cancellation.
    pub fn new(
        columns: Vec<Column>,
        availability: Vec<MemberAvailability>,
        names: NameComparison,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RecordsetError> {
        Self::with_kind(
            columns,
            availability,
            names,
            RecordsetKind::Rows,
            budget,
            cancelled,
        )
    }

    /// Validate exact member declarations with explicit source-declared child semantics.
    /// The kind neither changes carrier validation nor grants storage or action authority.
    ///
    /// # Errors
    /// Rejects empty schemas, duplicate names, unequal declaration/availability
    /// widths, exhausted work or cancellation, exactly as the generic constructor.
    pub fn with_kind(
        columns: Vec<Column>,
        availability: Vec<MemberAvailability>,
        names: NameComparison,
        kind: RecordsetKind,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RecordsetError> {
        check(budget, cancelled)?;
        if columns.len() != availability.len() {
            return Err(RecordsetError::Width);
        }
        validate_columns_with(&columns, names, || check(budget, cancelled))?;
        let column_bytes = columns
            .capacity()
            .checked_mul(size_of::<Column>())
            .ok_or(RecordsetError::Size)?;
        let availability_bytes = availability
            .capacity()
            .checked_mul(size_of::<MemberAvailability>())
            .ok_or(RecordsetError::Size)?;
        let mut retained_bytes = size_of::<Self>()
            .checked_add(column_bytes)
            .and_then(|bytes| bytes.checked_add(availability_bytes))
            .ok_or(RecordsetError::Size)?;
        for column in &columns {
            check(budget, cancelled)?;
            retained_bytes = retained_bytes
                .checked_add(column.schema_name().allocated_bytes())
                .ok_or(RecordsetError::Size)?;
        }
        check(budget, cancelled)?;
        Ok(Self {
            columns,
            availability,
            names,
            kind,
            retained_bytes,
        })
    }

    /// Borrow declarations in their exact stable source order, including uninspected members.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Borrow inspection policies in the same order and width as the declarations.
    pub fn availability(&self) -> &[MemberAvailability] {
        &self.availability
    }

    /// Exact caller-selected schema-name comparison, without inferred SQL collation.
    pub const fn names(&self) -> NameComparison {
        self.names
    }

    /// Source-declared semantics retained even for a present empty child recordset.
    pub const fn kind(&self) -> RecordsetKind {
        self.kind
    }

    /// Cached size of this owner and its buffers, including unused capacity.
    /// Shared-owner control blocks and allocator overhead are excluded.
    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
