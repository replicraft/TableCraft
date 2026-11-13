//! Draft ownership follows the scratch object, with bounded retained heap accounting.
use super::{PendingKind, QueryDesign, Session, baseline::Baseline};
use crate::workspace::objects::memory::Counter;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tablecraft_builders::query::{DesignExpression, DesignTableMetadata, QueryDesignDraft};

// Reserve strong/weak ownership counters and a full payload-alignment unit.
// This is conservative logical accounting, not an assumed private Arc ABI.
pub(super) const SHARED_DRAFT_CONTROL_BYTES: usize =
    size_of::<(AtomicUsize, AtomicUsize)>() + align_of::<QueryDesignDraft>();

impl QueryDesign {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let Some(session) = &self.session {
            count.add(size_of_val(session.as_ref()));
            count_draft(&mut count, &session.draft);
            count_snapshots(&mut count, session);
            session.field_inputs.count(&mut count);
            count_metadata(&mut count, &session.metadata);
            count.optional(&session.error);
            count.optional(&session.selected_table);
            if let Some(pending) = &session.pending {
                if let crate::workspace::ObjectKind::SavedQuery { name, .. } =
                    &pending.request.tab.target.kind
                {
                    count.string(name);
                }
                match &pending.kind {
                    PendingKind::Describe { table } => count.string(table),
                    PendingKind::Build { .. } => {}
                    PendingKind::Import { sql } | PendingKind::Run { sql } => count.string(sql),
                }
            }
        }
        count.finish()
    }
}

pub(super) fn count_draft(count: &mut Counter, draft: &QueryDesignDraft) {
    count.vector(&draft.parameters);
    for parameter in &draft.parameters {
        count.add(parameter.name.retained_text_bytes());
        count.string(&parameter.data_type.name);
        count.vector(&parameter.data_type.arguments);
    }
    count.vector(&draft.sources);
    for source in &draft.sources {
        count.string(&source.table);
        count.optional(&source.alias);
    }
    count.vector(&draft.joins);
    for join in &draft.joins {
        count.string(&join.left.column);
        count.string(&join.right.column);
    }
    count.vector(&draft.columns);
    for column in &draft.columns {
        match &column.expression {
            DesignExpression::Field(field) => count.string(&field.column),
            DesignExpression::Expression(text) => count.string(text),
            DesignExpression::AllFields(_) => {}
        }
        count.optional(&column.alias);
        count.vector(&column.criteria);
        for criterion in &column.criteria {
            count.string(criterion);
        }
    }
}

pub(super) fn count_shared_draft(count: &mut Counter, draft: &Arc<QueryDesignDraft>) {
    count.add(size_of::<QueryDesignDraft>() + SHARED_DRAFT_CONTROL_BYTES);
    count_draft(count, draft);
}

pub(super) fn shared_draft_bytes(draft: &Arc<QueryDesignDraft>) -> Option<usize> {
    let mut count = Counter::new(0);
    count_shared_draft(&mut count, draft);
    count.finish()
}

fn count_snapshots(count: &mut Counter, session: &Session) {
    let pending = match &session.pending {
        Some(super::Pending {
            kind: PendingKind::Build { draft, .. },
            ..
        }) => Some(draft),
        _ => None,
    };
    count_snapshot_set(
        count,
        &session.baseline,
        session.submitted.as_ref().map(|submitted| &submitted.draft),
        pending,
    );
}

fn count_snapshot_set(
    count: &mut Counter,
    baseline: &Baseline,
    submitted: Option<&Arc<QueryDesignDraft>>,
    pending: Option<&Arc<QueryDesignDraft>>,
) {
    let baseline = baseline.snapshot();
    if let Some(baseline) = baseline {
        count_shared_draft(count, baseline);
    }
    if let Some(submitted) = submitted
        && baseline.is_none_or(|baseline| !Arc::ptr_eq(baseline, submitted))
    {
        count_shared_draft(count, submitted);
    }
    if let Some(pending) = pending
        && baseline.is_none_or(|baseline| !Arc::ptr_eq(baseline, pending))
        && submitted.is_none_or(|submitted| !Arc::ptr_eq(submitted, pending))
    {
        count_shared_draft(count, pending);
    }
}

#[allow(clippy::ptr_arg)] // Count the retained allocation, including spare Vec capacity.
fn count_metadata(count: &mut Counter, metadata: &Vec<DesignTableMetadata>) {
    count.vector(metadata);
    for table in metadata {
        count.string(&table.table);
        count.vector(&table.fields);
        for field in &table.fields {
            count.string(&field.name);
        }
    }
}

#[allow(clippy::ptr_arg)] // The actual incoming metadata Vec allocation survives completion.
pub(super) fn completion_bytes(
    session: &Session,
    draft: &QueryDesignDraft,
    metadata: &Vec<DesignTableMetadata>,
    baseline: &Baseline,
    submitted: Option<&Arc<QueryDesignDraft>>,
) -> Option<usize> {
    let mut count = Counter::new(size_of::<QueryDesign>() + size_of::<Session>());
    count_draft(&mut count, draft);
    count_snapshot_set(&mut count, baseline, submitted, None);
    session.field_inputs.count_cleared(&mut count);
    count_metadata(&mut count, metadata);
    count.optional(&session.selected_table);
    count.finish()
}
