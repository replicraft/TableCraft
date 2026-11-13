//! Exact semantic baselines remain independent of request freshness revisions.
#[cfg(test)]
mod tests;
use super::{Mode, QueryDesign, Session, retention};
use crate::workspace::{
    State,
    objects::{MAX_RETAINED_BYTES, memory::Counter},
};
use std::sync::Arc;
use tablecraft_builders::query::{BuiltQueryDesign, DesignTableMetadata, QueryDesignDraft};

#[derive(Default)]
pub(super) enum Baseline {
    #[default]
    /// The untouched empty editor, not a native persisted object.
    Empty,
    /// Dirty saved SQL has not supplied an admitted stored-model comparison.
    Unknown,
    /// Exact imported clean-source or natively accepted typed model.
    Known(Arc<QueryDesignDraft>),
}

impl Baseline {
    pub(super) fn matches(&self, draft: &QueryDesignDraft) -> bool {
        match self {
            Self::Empty => *draft == QueryDesignDraft::default(),
            Self::Unknown => false,
            Self::Known(baseline) => baseline.as_ref() == draft,
        }
    }

    pub(super) fn snapshot(&self) -> Option<&Arc<QueryDesignDraft>> {
        match self {
            Self::Known(snapshot) => Some(snapshot),
            Self::Empty | Self::Unknown => None,
        }
    }

    pub(super) fn share(&self) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::Unknown => Self::Unknown,
            // NO_CLONE: Preferences and the live mutable editor retain distinct
            // handles to the same immutable comparison baseline during atomic rebind.
            Self::Known(snapshot) => Self::Known(Arc::clone(snapshot)),
        }
    }
}

impl Session {
    pub(super) fn shared_snapshot(
        &self,
        draft: &QueryDesignDraft,
    ) -> Option<Arc<QueryDesignDraft>> {
        // NO_CLONE: Reuse a real immutable accepted/baseline model instead of
        // copying its strings and vectors for another comparison owner.
        self.baseline
            .snapshot()
            .into_iter()
            .chain(self.submitted.as_ref().map(|submitted| &submitted.draft))
            .find(|snapshot| snapshot.as_ref() == draft)
            .map(Arc::clone)
    }
}

impl State {
    pub(super) fn capture_query_design_baseline(
        &mut self,
        draft: &QueryDesignDraft,
    ) -> Option<Arc<QueryDesignDraft>> {
        let mut count =
            Counter::new(size_of::<QueryDesignDraft>() + retention::SHARED_DRAFT_CONTROL_BYTES);
        retention::count_draft(&mut count, draft);
        if !self.admit_object_bytes(count.finish()) {
            return None;
        }

        // NO_CLONE: A newly imported model has no immutable submitted owner yet.
        // Its exact baseline must survive future independent mutation of the live
        // editor; borrowing cannot cover that lifetime. Later builds/rebinds share it.
        let snapshot = Arc::new(draft.clone());

        let mut actual = Counter::new(0);
        retention::count_shared_draft(&mut actual, &snapshot);
        self.admit_object_bytes(actual.finish()).then_some(snapshot)
    }
}

impl QueryDesign {
    pub(in crate::workspace) fn dirty_revision(&self) -> Option<u64> {
        let session = self.session.as_ref()?;
        (session.field_inputs.unresolved()
            || (session.mode != Mode::Sql && !session.baseline.matches(&session.draft)))
        .then_some(session.revision)
    }

    pub(in crate::workspace) fn is_dirty(&self) -> bool {
        self.dirty_revision().is_some()
    }

    /// Idempotent observation used after the actual saved publication has
    /// installed its baseline. Never promotes unbuilt authoring data.
    pub(in crate::workspace) fn saved_baseline_current(&self) -> bool {
        self.session.as_ref().is_none_or(|session| {
            !session.field_inputs.unresolved() && session.baseline.matches(&session.draft)
        })
    }

    /// The caller supplies the accepted SQL revision only after the actual
    /// native query/full-witness publication. A hidden model from another SQL
    /// revision is not evidence for this save.
    pub(in crate::workspace) fn resolve_saved_text(&mut self, accepted_revision: u64) -> bool {
        let Some(session) = &mut self.session else {
            return true;
        };
        if !session.field_inputs.unresolved()
            && session.submitted.as_ref().is_some_and(|submitted| {
                submitted.text_revision == accepted_revision
                    && submitted.draft.as_ref() == &session.draft
            })
        {
            let Some(submitted) = session.submitted.take() else {
                return false;
            };
            session.baseline = Baseline::Known(submitted.draft);
            return true;
        }

        if session.mode == Mode::Sql {
            // SQL View superseded this hidden model. Reimport the actual current
            // authored SQL before Design can be used on the new source.
            self.session = None;
            return true;
        }
        false
    }
}

impl Session {
    pub(super) fn owns_snapshot(&self, draft: &Arc<QueryDesignDraft>) -> bool {
        self.baseline
            .snapshot()
            .is_some_and(|baseline| Arc::ptr_eq(baseline, draft))
            || self
                .submitted
                .as_ref()
                .is_some_and(|submitted| Arc::ptr_eq(&submitted.draft, draft))
    }
}

impl State {
    pub(super) fn query_design_snapshot(&self) -> Option<Arc<QueryDesignDraft>> {
        let session = self.query_design.session.as_ref()?;
        if let Some(snapshot) = session.shared_snapshot(&session.draft) {
            return Some(snapshot);
        }

        let mut estimate = Counter::new(self.object_bytes()?);
        estimate.add(size_of::<QueryDesignDraft>() + retention::SHARED_DRAFT_CONTROL_BYTES);
        retention::count_draft(&mut estimate, &session.draft);
        if estimate.finish()? > MAX_RETAINED_BYTES {
            return None;
        }

        // NO_CLONE: The first submission of a changed mutable model needs an
        // immutable worker snapshot. Subsequent equal builds reuse this allocation;
        // borrowing the editor cannot outlive later user edits on the UI thread.
        let snapshot = Arc::new(session.draft.clone());
        let mut actual = Counter::new(self.object_bytes()?);
        retention::count_shared_draft(&mut actual, &snapshot);
        (actual.finish()? <= MAX_RETAINED_BYTES).then_some(snapshot)
    }
}

/// A real accepted Build and the exact authoring text epoch it published.
pub(super) struct Submission {
    pub(super) draft: Arc<QueryDesignDraft>,
    pub(super) text_revision: u64,
}

impl Submission {
    pub(super) fn share(&self) -> Self {
        // NO_CLONE: Fresh-source preferences retain the immutable accepted
        // model while the live session still owns its separate handle.
        Self {
            draft: Arc::clone(&self.draft),
            text_revision: self.text_revision,
        }
    }
}

impl State {
    #[allow(clippy::ptr_arg)] // The actual incoming metadata Vec capacity is retained after publication.
    pub(super) fn admit_query_design_completion(
        &mut self,
        built: &BuiltQueryDesign,
        metadata: &Vec<DesignTableMetadata>,
        baseline: &Baseline,
        submitted: Option<&Arc<QueryDesignDraft>>,
        imported: bool,
    ) -> bool {
        let admitted = (|| {
            let session = self.query_design.session.as_ref()?;
            let draft = if imported {
                &built.draft
            } else {
                &session.draft
            };

            let proposed_design =
                retention::completion_bytes(session, draft, metadata, baseline, submitted)?;
            let mut total = self
                .object_bytes()?
                .checked_sub(self.query_design.retained_bytes()?)?
                .checked_add(proposed_design)?;

            if self.query_design_sql()? != built.sql {
                let prior = if self.saved_query.active {
                    self.saved_query.sql.as_ref()?.text_capacity()
                } else {
                    self.sql.capacity()
                };
                total = total
                    .checked_sub(prior)?
                    .checked_add(built.sql.capacity())?;
            }

            Some(total <= MAX_RETAINED_BYTES)
        })()
        .unwrap_or(false);

        if !admitted {
            self.error = Some(self.localizer.text("objects-memory-limit"));
        }
        admitted
    }
}
