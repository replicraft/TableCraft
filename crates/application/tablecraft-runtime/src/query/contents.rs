//! Issued read-only query results retain child metadata independently of native rows.
mod inspect;
mod retention;

use crate::{
    Cell, CellKind, Error, HorizontalAlignment, display::Presentation, document::Document,
};
use std::sync::Arc;
use tablecraft_db::{OwnedRecordset, RecordsetPresence};

const ARC_COUNTERS: usize = 2 * size_of::<usize>();

/// One result-local child binding. Coordinates address output cells, not native rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryCollectionBinding {
    /// Zero-based output row within this complete query result.
    pub row: usize,
    /// Zero-based output column, independent of native field numbers.
    pub column: usize,
    /// Checked slot within the private issued metadata owner.
    pub slot: usize,
}

/// Runtime-issued immutable child metadata. No public constructor or native bookmark.
/// Cloning shares the result and its already-owned child snapshots.
#[derive(Clone)]
pub struct QueryResultHandle {
    state: Arc<ResultState>,
}

struct ResultState {
    source: Arc<()>,
    revision: u64,
    bindings: Vec<Binding>,
    retained_bytes: usize,
}

struct Binding {
    row: usize,
    column: usize,
    value: Arc<OwnedRecordset>,
}

pub(crate) struct QueryResultIssuer {
    source: Arc<()>,
    revision: u64,
}

pub(crate) struct QueryContentsBuilder {
    issuer: QueryResultIssuer,
    bindings: Vec<Binding>,
}

impl Document {
    pub(crate) fn query_result_issuer(&self) -> QueryResultIssuer {
        QueryResultIssuer {
            // NO_CLONE: Keep the small issuing-source identity while the document
            // and independently published result outlive each other's borrows.
            source: Arc::clone(&self.filter_source),
            revision: self.revision,
        }
    }
}

impl QueryResultIssuer {
    pub(crate) fn builder(self) -> QueryContentsBuilder {
        QueryContentsBuilder {
            issuer: self,
            bindings: Vec::new(),
        }
    }
}

impl QueryResultHandle {
    /// Borrow inert output coordinates without exposing child payloads or native identity.
    pub fn bindings(&self) -> impl ExactSizeIterator<Item = QueryCollectionBinding> + '_ {
        self.state
            .bindings
            .iter()
            .enumerate()
            .map(|(slot, binding)| QueryCollectionBinding {
                row: binding.row,
                column: binding.column,
                slot,
            })
    }

    /// Whether two handles share this exact successful result owner.
    pub fn shares_result(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    /// Cached retained metadata, counting each snapshot and schema allocation once.
    /// Includes binding capacity and Arc counters, but not the shared issuing-source
    /// identity or allocator-specific bookkeeping. Inspection never walks child data
    /// merely to report this footprint.
    pub fn retained_bytes(&self) -> usize {
        self.state.retained_bytes
    }

    pub(crate) fn document_revision(&self) -> u64 {
        self.state.revision
    }
}

impl QueryContentsBuilder {
    pub(crate) fn absent_recordset_cell(output: &mut Presentation<'_>) -> Result<Cell, Error> {
        Ok(Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: output.text("No parent record")?,
            kind: CellKind::AbsentCollection,
        })
    }

    pub(crate) fn recordset_cell(
        &mut self,
        row: usize,
        column: usize,
        value: Arc<OwnedRecordset>,
        output: &mut Presentation<'_>,
    ) -> Result<Cell, Error> {
        output.work(1)?;
        if self
            .bindings
            .last()
            .is_some_and(|last| (last.row, last.column) >= (row, column))
        {
            return Err(Error::QueryContentsUnavailable);
        }
        let assigned = value.presence() == RecordsetPresence::Assigned;
        let members = value.len();
        let text = if assigned {
            output.text(&format!(
                "{members} {}",
                if members == 1 {
                    "attachment"
                } else {
                    "attachments"
                }
            ))?
        } else {
            output.text("NULL")?
        };
        output.allocation(1, size_of::<Binding>())?;
        let previous = self.bindings.capacity();
        self.bindings.try_reserve(1)?;
        let extra = self
            .bindings
            .capacity()
            .saturating_sub(previous)
            .saturating_sub(1);
        output.allocation(extra, size_of::<Binding>())?;
        self.bindings.push(Binding { row, column, value });
        Ok(Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text,
            kind: CellKind::Attachments { assigned, members },
        })
    }

    pub(crate) fn finish(
        self,
        output: &mut Presentation<'_>,
    ) -> Result<Option<QueryResultHandle>, Error> {
        if self.bindings.is_empty() {
            return Ok(None);
        }
        let mut bytes = size_of::<ResultState>()
            .checked_add(ARC_COUNTERS)
            .and_then(|bytes| {
                bytes.checked_add(self.bindings.capacity().checked_mul(size_of::<Binding>())?)
            })
            .ok_or(Error::QueryContentsSize)?;
        bytes = bytes
            .checked_add(retention::payload_bytes(&self.bindings, output)?)
            .ok_or(Error::QueryContentsSize)?;
        output.allocation(1, size_of::<ResultState>() + ARC_COUNTERS)?;
        Ok(Some(QueryResultHandle {
            state: Arc::new(ResultState {
                source: self.issuer.source,
                revision: self.issuer.revision,
                bindings: self.bindings,
                retained_bytes: bytes,
            }),
        }))
    }
}

#[cfg(test)]
mod tests;
