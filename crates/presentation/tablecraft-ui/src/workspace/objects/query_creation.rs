//! Exact active or parked authoring association for query-created publication.
use crate::{QueryCreationRequest, State};

impl State {
    pub(in crate::workspace) fn query_creation_owner_current(
        &self,
        request: &QueryCreationRequest,
    ) -> bool {
        if self
            .database
            .as_ref()
            .is_none_or(|database| database.document_revision != request.document_revision)
        {
            return false;
        }
        if self.active_object().as_ref() == Some(&request.tab) {
            return self.query_creation_request_current(request);
        }
        let Some(entry) = self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab == request.tab)
        else {
            return false;
        };
        match &entry.session {
            Some(super::session::Session::Sql { design, .. }) => {
                request.source.is_none()
                    && self.sql == request.sql.as_str()
                    && self.query_sql_revision() == Some(request.text_revision)
                    && design.save_revision() == request.design_revision
            }
            Some(super::session::Session::Saved { state, .. }) => {
                request.source.as_ref() == state.target.as_ref()
                    && state.sql.as_ref().is_some_and(|draft| {
                        draft.revision() == request.text_revision
                            && draft.text() == request.sql.as_str()
                    })
                    && state.design.save_revision() == request.design_revision
            }
            _ => false,
        }
    }
}
