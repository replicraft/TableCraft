//! Only visibility and field choices survive a fresh database revision.
use super::{MAX_TOTAL_FIELDS, Session, TotalSpec, Totals};
use crate::workspace::objects::memory::Counter;

#[derive(Clone)]
pub(in crate::workspace) struct TotalsPreferences {
    visible: bool,
    specs: Vec<TotalSpec>,
}

impl Totals {
    pub(in crate::workspace) fn preferences(&self) -> Option<TotalsPreferences> {
        self.session.as_ref().map(|session| TotalsPreferences {
            visible: session.visible,
            // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the original
            // live declarations, draft, tab or totals preferences remain in use;
            // parameter edit text and declaration identity are separate owners.
            specs: session.specs.clone(),
        })
    }

    pub(in crate::workspace) fn restore(preferences: Option<TotalsPreferences>) -> Self {
        let Some(preferences) = preferences else {
            return Self::default();
        };
        if preferences.specs.len() > MAX_TOTAL_FIELDS {
            return Self::default();
        }
        Self {
            session: Some(Box::new(Session {
                visible: preferences.visible,
                specs: preferences.specs,
                ..Session::default()
            })),
        }
    }

    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let Some(session) = &self.session {
            count.add(size_of::<Session>());
            if let Some(target) = &session.target {
                count.string(&target.table);
            }
            count.vector(&session.specs);
            for spec in &session.specs {
                count.string(&spec.column);
            }
            if let Some(description) = &session.description {
                count.string(&description.target.table);
                count.vector(&description.fields);
                for field in &description.fields {
                    count.string(&field.column.name);
                    count.optional(&field.column.caption);
                    count.vector(&field.aggregates);
                }
            }
            if let Some(result) = &session.result {
                count.string(&result.target.table);
                count.vector(&result.specs);
                for spec in &result.specs {
                    count.string(&spec.column);
                }
                count.vector(&result.cells);
                for cell in &result.cells {
                    count.string(&cell.text);
                }
            }
            if let Some(pending) = &session.pending {
                count.optional(&pending.formatting.system_locale);
                if let Some(specs) = &pending.specs {
                    count.vector(specs);
                    for spec in specs {
                        count.string(&spec.column);
                    }
                }
                if let crate::ObjectKind::Table { name } = &pending.object.target.kind {
                    count.string(name);
                }
            }
            count.optional(&session.error);
            count.optional(&session.selected_column);
        }
        count.finish()
    }

    pub(in crate::workspace) fn apply_display(&mut self, cache: &mut crate::formatting::Cache) {
        if let Some(result) = self
            .session
            .as_mut()
            .and_then(|session| session.result.as_mut())
        {
            for cell in &mut result.cells {
                cache.apply(cell);
            }
        }
    }
}
