//! Exact close-objects approval is independent of native setting facts/capabilities.
use super::{DatabaseOptionsCloseRequest, Phase};
use crate::workspace::{ObjectKind, State};

impl State {
    pub fn request_database_options_close_confirmation(&mut self) -> bool {
        if !self.can_disable_name_auto_correct() {
            return false;
        }
        let Some(next) = self.database_options_confirmation_sequence.checked_add(1) else {
            return false;
        };
        let Some(graph) = self.capture_database_options_close_graph(next) else {
            return false;
        };
        self.database_options_confirmation_sequence = next;
        self.database_options.confirmation = Some(graph);
        self.database_options.phase = Phase::Confirming;
        true
    }

    pub fn database_options_close_graph_current(&self) -> bool {
        self.database_options
            .confirmation
            .as_ref()
            .is_some_and(|graph| graph.approved && self.database_options_graph_matches(graph))
    }

    pub fn approve_database_options_close(&mut self) -> Option<DatabaseOptionsCloseRequest> {
        if self.database_options.phase != Phase::Confirming {
            return None;
        }
        let graph = self.database_options.confirmation.as_ref()?;
        if !self.database_options_graph_matches(graph)
            || !self.can_rebind_database_options_layout(graph.request.document_revision)
        {
            return None;
        }
        let graph = self.database_options.confirmation.as_mut()?;
        graph.approved = true;
        let request = graph.request;
        self.database_options.phase = Phase::Idle;
        self.options_open = false;
        Some(request)
    }

    pub fn cancel_database_options_close(&mut self) -> bool {
        if self.database_options.phase != Phase::Confirming {
            return false;
        }
        self.database_options.confirmation = None;
        self.database_options.phase = Phase::Idle;
        true
    }

    pub fn database_options_close_request_current(
        &self,
        request: DatabaseOptionsCloseRequest,
    ) -> bool {
        self.database_options
            .confirmation
            .as_ref()
            .is_some_and(|graph| graph.request == request)
            && self.database_options_close_graph_current()
    }

    pub(in crate::workspace) fn database_options_close_names(&self) -> impl Iterator<Item = &str> {
        self.database_options
            .confirmation
            .iter()
            .flat_map(|graph| &graph.tabs)
            .filter_map(|tab| match &tab.target.kind {
                ObjectKind::Table { name } | ObjectKind::SavedQuery { name, .. } => {
                    Some(name.as_str())
                }
                _ => None,
            })
    }
}
