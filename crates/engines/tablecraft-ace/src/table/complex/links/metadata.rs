use super::super::{ComplexColumnSchema, ComplexTableSchema};
use crate::TableSnapshot;
use std::ops::Range;
use tablecraft_core::values::Value;

/// One snapshot-local parent row's stored key and checked child membership.
pub struct ComplexParentLink {
    pub(super) key: Option<i32>,
    pub(super) range: Range<usize>,
}
impl ComplexParentLink {
    /// Raw key scoped to this snapshot's selected column; not a global object ID.
    pub const fn key(&self) -> Option<i32> {
        self.key
    }
    /// Number of checked flat child records bound to this parent row.
    pub fn child_count(&self) -> usize {
        self.range.len()
    }
}

/// Completed explicit raw-storage view; no query flattening, attachment decoding,
/// history ordering, editing or Access collection semantics are implied.
pub struct ComplexColumnLinks {
    pub(super) schema: ComplexTableSchema,
    pub(super) position: usize,
    pub(super) parents: Vec<ComplexParentLink>,
    pub(super) children: TableSnapshot,
    pub(super) order: Vec<(usize, usize)>,
}
impl ComplexColumnLinks {
    /// Checked owner and complex type/flat schema declarations.
    pub fn schema(&self) -> &ComplexTableSchema {
        &self.schema
    }
    /// Selected complex declaration; keys belong only to this column.
    pub fn column(&self) -> &ComplexColumnSchema {
        &self.schema.columns()[self.position]
    }
    /// Parent links in sequential live-root order, including empty memberships.
    pub fn parents(&self) -> &[ComplexParentLink] {
        &self.parents
    }
    /// Complete validated raw flat child snapshot, retaining encoded Binary.
    pub fn child_snapshot(&self) -> &TableSnapshot {
        &self.children
    }
    /// Checked children in their flat snapshot order, or None for an absent row
    /// ordinal. An existing parent with no children returns an empty iterator.
    pub fn child_rows(&self, parent: usize) -> Option<impl ExactSizeIterator<Item = &[Value]>> {
        let range = self.parents.get(parent)?.range.clone();
        Some(
            self.order[range]
                .iter()
                .map(|(_, row)| self.children.rows()[*row].as_slice()),
        )
    }
}
