//! Source-observed saved-table naming. This state never grants native authority.
//!
//! The host owns source inspection, consuming reviews, worker resources and the
//! complete AFTER witness. The datasheet renders this name separately from its
//! stored columns; merely naming a field must not invent a schema or a record.
mod abandon;
mod close;
mod dialog;
pub(crate) mod header;
mod inspection_completion;
mod state;
mod status;

use abandon::FieldAdditionSavedWarning;
pub use close::{FieldAdditionCloseDecision, FieldAdditionCloseResult};
pub use tablecraft_builders::table::{FieldAdditionDeclaration, FieldAdditionKind};

pub(super) use status::Status;
pub use status::{FieldAdditionSaveCommand, FieldAdditionSaveIdentity, PreparedFieldAdditionSave};

use super::{ObjectKind, ObjectTab};
use tablecraft_builders::table::{MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

/// Exact naming snapshot submitted after the current frame's header widgets.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldAdditionRequest {
    pub declaration: FieldAdditionDeclaration,
    pub tab: ObjectTab,
    pub draft_revision: u64,
    pub column: String,
}

/// Source inspection admission failure; an existing pending owner is retained.
/// Source-observed gallery facts. These never substitute for a consuming review.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldAdditionChoices {
    pub number: bool,
    pub short_text: bool,
}

impl FieldAdditionChoices {
    pub const fn supports(self, kind: FieldAdditionKind) -> bool {
        match kind {
            FieldAdditionKind::Number => self.number,
            FieldAdditionKind::ShortText => self.short_text,
        }
    }

    pub const fn any(self) -> bool {
        self.number || self.short_text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionInspectionError {
    Unavailable,
    AlreadyObserved,
    Capacity,
    Allocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Naming,
    Reviewing {
        request_id: u64,
    },
    Reviewed {
        request_id: u64,
        review_id: u64,
    },
    Saving {
        request_id: u64,
        review_id: u64,
        save_id: u64,
    },
}

pub(super) struct Draft {
    declaration: FieldAdditionDeclaration,
    tab: ObjectTab,
    column: String,
    revision: u64,
    phase: Phase,
    focus: bool,
}

/// One outstanding observation, bound to the exact table session and host ID.
pub(super) struct Inspection {
    tab: ObjectTab,
    request_id: Option<u64>,
    available: Option<FieldAdditionChoices>,
    error: Option<String>,
    interrupted: bool,
}

impl Inspection {
    fn pending(&self) -> bool {
        self.available.is_none() && self.error.is_none() && !self.interrupted
    }
}

#[derive(Default)]
pub(super) struct Model {
    inspections: Vec<Inspection>,
    draft: Option<Draft>,
    close: Option<close::Confirmation>,
    warning: Option<abandon::SavedWarning>,
}

impl Model {
    /// Call only for a current local user-table target. The State connector must
    /// also enforce host support, modal/dirty gates and retained-byte admission.
    #[cfg(test)]
    pub(super) fn inspect(
        &mut self,
        tab: ObjectTab,
        request_id: u64,
    ) -> Result<(), FieldAdditionInspectionError> {
        self.prepare_inspection(tab.clone())?;
        assert!(self.bind_inspection(&tab, request_id));
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn prepare_inspection(
        &mut self,
        tab: ObjectTab,
    ) -> Result<(), FieldAdditionInspectionError> {
        let storage = self.reserve_inspection(&tab)?;
        self.install_inspection(storage, tab)
    }

    /// Reserve independent storage so the owner can admit its ACTUAL capacity
    /// before moving any existing observation or publishing the new slot.
    pub(super) fn reserve_inspection(
        &self,
        tab: &ObjectTab,
    ) -> Result<Vec<Inspection>, FieldAdditionInspectionError> {
        if self.draft.is_some() || !matches!(tab.target.kind, ObjectKind::Table { .. }) {
            return Err(FieldAdditionInspectionError::Unavailable);
        }
        if self
            .inspections
            .iter()
            .any(|inspection| inspection.tab == *tab)
        {
            return Err(FieldAdditionInspectionError::AlreadyObserved);
        }
        if self.inspections.len() >= super::objects::MAX_OBJECTS {
            return Err(FieldAdditionInspectionError::Capacity);
        }
        let mut storage = Vec::new();
        storage
            .try_reserve_exact(self.inspections.len() + 1)
            .map_err(|_| FieldAdditionInspectionError::Allocation)?;
        Ok(storage)
    }

    pub(super) fn install_inspection(
        &mut self,
        mut storage: Vec<Inspection>,
        tab: ObjectTab,
    ) -> Result<(), FieldAdditionInspectionError> {
        if !storage.is_empty()
            || storage.capacity() <= self.inspections.len()
            || !self.needs_inspection(&tab)
        {
            return Err(FieldAdditionInspectionError::Unavailable);
        }
        storage.append(&mut self.inspections);
        storage.push(Inspection {
            tab,
            request_id: None,
            available: None,
            error: None,
            interrupted: false,
        });
        self.inspections = storage;
        Ok(())
    }

    pub(super) fn bind_inspection(&mut self, tab: &ObjectTab, request_id: u64) -> bool {
        let Some(inspection) = self
            .inspections
            .iter_mut()
            .find(|item| item.tab == *tab && item.request_id.is_none() && item.pending())
        else {
            return false;
        };
        inspection.request_id = Some(request_id);
        true
    }

    /// Keep pending requests even if their tab closed: their exact terminal
    /// response must resolve before their cache slot may be retired.
    pub(super) fn prune_inspections(&mut self, mut live: impl FnMut(&ObjectTab) -> bool) {
        self.inspections
            .retain(|item| live(&item.tab) || (item.pending()));
    }

    pub(super) fn needs_inspection(&self, tab: &ObjectTab) -> bool {
        !self
            .inspections
            .iter()
            .any(|inspection| inspection.tab == *tab)
    }

    pub(super) fn retry_inspection(&mut self, tab: &ObjectTab) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let Some(position) = self
            .inspections
            .iter()
            .position(|inspection| inspection.tab == *tab && !inspection.pending())
        else {
            return false;
        };
        self.inspections.remove(position);
        true
    }

    pub(super) fn observed(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
        number: bool,
        short_text: bool,
    ) -> bool {
        let Some(inspection) = self.inspections.iter_mut().find(|inspection| {
            inspection.tab == *tab
                && inspection.request_id == Some(request_id)
                && inspection.pending()
        }) else {
            return false;
        };
        inspection.available = Some(FieldAdditionChoices { number, short_text });
        true
    }

    pub(super) fn inspection_failed(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
        message: String,
    ) -> bool {
        let Some(inspection) = self.inspections.iter_mut().find(|inspection| {
            inspection.tab == *tab
                && inspection.request_id == Some(request_id)
                && inspection.pending()
        }) else {
            return false;
        };
        inspection.error = Some(message);
        true
    }

    pub(super) fn available(&self, tab: &ObjectTab, kind: FieldAdditionKind) -> bool {
        self.draft.is_none()
            && self.inspections.iter().any(|inspection| {
                inspection.tab == *tab
                    && inspection
                        .available
                        .is_some_and(|choices| choices.supports(kind))
            })
    }

    /// `column` is the proposed header spelling, without trimming or coercion.
    /// Name semantics and collisions are checked by the actual native review.
    pub(super) fn begin(
        &mut self,
        tab: &ObjectTab,
        column: String,
        declaration: FieldAdditionDeclaration,
    ) -> bool {
        if !self.available(tab, declaration.kind()) || !bounded_name(&column) {
            return false;
        }
        let Some(tab) = copy_tab(tab) else {
            return false;
        };
        self.draft = Some(Draft {
            declaration,
            tab,
            column,
            revision: 0,
            phase: Phase::Naming,
            focus: true,
        });
        true
    }

    pub(super) fn presented_draft(&self) -> Option<&Draft> {
        self.draft.as_ref().or_else(|| {
            self.warning
                .as_ref()
                .and_then(abandon::SavedWarning::presented)
        })
    }

    pub(super) fn draft(&self) -> Option<(&ObjectTab, &str, u64, Phase)> {
        let draft = self.presented_draft()?;
        Some((&draft.tab, &draft.column, draft.revision, draft.phase))
    }

    pub(super) fn replace_name(&mut self, tab: &ObjectTab, column: String) -> bool {
        let Some(draft) = &mut self.draft else {
            return false;
        };
        if draft.tab != *tab || draft.phase != Phase::Naming || !bounded_name(&column) {
            return false;
        }
        if draft.column != column {
            let Some(revision) = draft.revision.checked_add(1) else {
                return false;
            };
            draft.column = column;
            draft.revision = revision;
        }
        true
    }

    pub(super) fn request(&self, tab: &ObjectTab) -> Option<FieldAdditionRequest> {
        let draft = self.draft.as_ref()?;
        if draft.tab != *tab || draft.phase != Phase::Naming || draft.column.is_empty() {
            return None;
        }
        Some(FieldAdditionRequest {
            declaration: draft.declaration,
            tab: copy_tab(&draft.tab)?,
            draft_revision: draft.revision,
            column: copy_text(&draft.column)?,
        })
    }

    pub(super) fn begin_review(&mut self, request: &FieldAdditionRequest, request_id: u64) -> bool {
        if !self.matches(request, Phase::Naming) {
            return false;
        }
        self.draft.as_mut().unwrap().phase = Phase::Reviewing { request_id };
        true
    }

    pub(super) fn reviewed(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        if !self.matches(request, Phase::Reviewing { request_id }) {
            return false;
        }
        self.draft.as_mut().unwrap().phase = Phase::Reviewed {
            request_id,
            review_id,
        };
        true
    }

    /// Bind an actual accepted worker ID, without allocating after acceptance.
    pub(super) fn saving(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
        review_id: u64,
        save_id: u64,
    ) -> bool {
        if !self.matches(
            request,
            Phase::Reviewed {
                request_id,
                review_id,
            },
        ) {
            return false;
        }
        self.draft.as_mut().unwrap().phase = Phase::Saving {
            request_id,
            review_id,
            save_id,
        };
        true
    }

    /// A refused/cancelled review grants no save authority and keeps exact text.
    pub(super) fn release_review(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
    ) -> bool {
        if !self.matches(request, Phase::Reviewing { request_id }) {
            return false;
        }
        self.draft.as_mut().unwrap().phase = Phase::Naming;
        true
    }

    pub(super) fn discard(&mut self, tab: &ObjectTab, revision: u64) -> bool {
        if self.draft.is_none() {
            return self
                .warning
                .as_mut()
                .is_some_and(|warning| warning.discard(tab, revision));
        }
        if !self.draft.as_ref().is_some_and(|draft| {
            draft.tab == *tab && draft.revision == revision && draft.phase == Phase::Naming
        }) {
            return false;
        }
        self.draft = None;
        true
    }

    fn matches(&self, request: &FieldAdditionRequest, phase: Phase) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft.declaration == request.declaration
                && draft.tab == request.tab
                && draft.revision == request.draft_revision
                && draft.column == request.column
                && draft.phase == phase
        })
    }

    /// Includes owned capacities. The State connector must count this model and
    /// any outgoing snapshot together before publishing either allocation.
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut bytes = size_of::<Self>();
        bytes = bytes.checked_add(
            self.inspections
                .capacity()
                .checked_mul(size_of::<Inspection>())?,
        )?;
        for inspection in &self.inspections {
            bytes = bytes
                .checked_add(table_name(&inspection.tab)?.capacity())?
                .checked_add(inspection.error.as_ref().map_or(0, String::capacity))?;
        }
        if let Some(draft) = &self.draft {
            bytes = bytes
                .checked_add(table_name(&draft.tab)?.capacity())?
                .checked_add(draft.column.capacity())?;
        }
        if let Some(warning) = &self.warning {
            bytes = bytes.checked_add(warning.heap_bytes()?)?;
        }
        if let Some(close) = &self.close {
            bytes = bytes.checked_add(close.heap_bytes()?)?;
        }
        Some(bytes)
    }
}

impl FieldAdditionRequest {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        size_of::<Self>()
            .checked_add(table_name(&self.tab)?.capacity())?
            .checked_add(self.column.capacity())
    }
}

fn bounded_name(name: &str) -> bool {
    name.len() <= MAX_DRAFT_NAME_BYTES
        && name.encode_utf16().take(MAX_DRAFT_NAME_UNITS + 1).count() <= MAX_DRAFT_NAME_UNITS
}

fn table_name(tab: &ObjectTab) -> Option<&String> {
    match &tab.target.kind {
        ObjectKind::Table { name } => Some(name),
        _ => None,
    }
}

fn copy_tab(tab: &ObjectTab) -> Option<ObjectTab> {
    Some(ObjectTab {
        id: tab.id,
        target: super::ObjectTarget {
            document_revision: tab.target.document_revision,
            kind: ObjectKind::Table {
                name: copy_text(table_name(tab)?)?,
            },
        },
    })
}

fn copy_text(text: &str) -> Option<String> {
    let mut result = String::new();
    result.try_reserve_exact(text.len()).ok()?;
    result.push_str(text);
    Some(result)
}

#[cfg(test)]
mod tests;

fn field_label_key(kind: FieldAdditionKind) -> &'static str {
    match kind {
        FieldAdditionKind::Number => "table-draft-number",
        FieldAdditionKind::ShortText => "table-draft-short-text",
    }
}
