//! Retain issued query handles under the actual published object and generation.
use super::{Desktop, PendingOperation};
use tablecraft_runtime::{Command, QueryResultHandle};
use tablecraft_ui::{
    Data, ObjectTab, QueryCollectionBinding, QueryContentsBindings, QueryContentsTarget, State,
};

#[derive(Default)]
pub(super) struct Publications {
    entries: Vec<Publication>,
}

struct Publication {
    tab: ObjectTab,
    generation: u64,
    handle: QueryResultHandle,
}

impl Publications {
    pub(super) fn prune(&mut self, state: &State) {
        self.entries
            .retain(|entry| state.object_tab_is_current(&entry.tab));
    }

    pub(super) fn prepare(&mut self) -> bool {
        self.entries.try_reserve(1).is_ok()
    }
}

impl Desktop {
    pub(super) fn query_contents_bindings(
        &mut self,
        generation: u64,
        handle: Option<&QueryResultHandle>,
    ) -> Result<Option<QueryContentsBindings>, ()> {
        let Some(handle) = handle else {
            return Ok(None);
        };
        if !self.query_contents.prepare() {
            return Err(());
        }
        let bindings = handle.bindings();
        let mut entries = Vec::new();
        entries.try_reserve_exact(bindings.len()).map_err(|_| ())?;
        for entry in bindings {
            entries.push(QueryCollectionBinding {
                row: entry.row,
                column: entry.column,
                slot: entry.slot,
            });
        }
        QueryContentsBindings::new(generation, entries, handle.retained_bytes())
            .map(Some)
            .ok_or(())
    }

    pub(super) fn retain_query_contents(
        &mut self,
        tab: ObjectTab,
        generation: u64,
        handle: Option<QueryResultHandle>,
    ) {
        if !self.state.object_tab_is_current(&tab)
            || self.state.active_object().as_ref() != Some(&tab)
        {
            return;
        }
        let current = self.state.query_contents_result_id();
        match handle {
            Some(handle) if current == Some(generation) => {
                self.query_contents.entries.retain(|entry| entry.tab != tab);
                self.query_contents.entries.push(Publication {
                    tab,
                    generation,
                    handle,
                });
            }
            None if current.is_none() => {
                self.query_contents.entries.retain(|entry| entry.tab != tab)
            }
            _ => {}
        }
    }

    pub(super) fn query_contents_target(
        &self,
        handle: &QueryResultHandle,
        binding: usize,
    ) -> Option<QueryContentsTarget> {
        let tab = self.state.active_object()?;
        let entry = self.query_contents.entries.iter().find(|entry| {
            entry.tab == tab
                && self.state.object_tab_is_current(&entry.tab)
                && entry.handle.shares_result(handle)
        })?;
        let target = QueryContentsTarget {
            result_id: entry.generation,
            binding,
        };
        self.state
            .can_view_query_contents(&target)
            .then_some(target)
    }

    pub(super) fn view_query_contents(&mut self, target: QueryContentsTarget) {
        self.query_contents.prune(&self.state);
        if !self.state.can_view_query_contents(&target) {
            return;
        }
        let Some(tab) = self.state.active_object() else {
            return;
        };
        let Some(entry) = self
            .query_contents
            .entries
            .iter()
            .find(|entry| entry.tab == tab && entry.generation == target.result_id)
        else {
            self.state.error = Some(self.state.localizer.text("collections-stale-view"));
            return;
        };
        // NO_CLONE: The read worker independently retains the issued immutable
        // result while the current and parked publication keeps its own handle.
        let result = entry.handle.clone();
        self.submit(Command::InspectQueryCollection {
            result,
            binding: target.binding,
        });
    }

    pub(super) fn publish_query_contents(
        &mut self,
        generation: u64,
        binding: usize,
        columns: Vec<tablecraft_runtime::Column>,
        rows: Vec<Vec<tablecraft_runtime::Cell>>,
    ) {
        let PendingOperation::QueryContents { request_id, target } = &self.pending else {
            return;
        };
        if *request_id != generation || target.binding != binding {
            return;
        }
        let target = *target;
        self.state.set_query_contents(
            generation,
            &target,
            Data {
                query_contents: None,
                source_revision: None,
                row_ordinals: None,
                table: self.state.localizer.text("collections-attachments-title"),
                offset: 0,
                columns: crate::view_models::columns(columns),
                rows: crate::view_models::rows(rows),
                has_more: false,
            },
        );
    }
}
