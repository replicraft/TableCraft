//! Query-design requests bridge shared editor state to headless runtime services.
use super::{Desktop, PendingOperation};
use crate::workspace::request_read;
use std::sync::Arc;
use tablecraft_builders::query::{BuiltQueryDesign, DesignTableMetadata, QueryDesignDraft};
use tablecraft_runtime::{self as runtime, Command};
use tablecraft_ui::{QueryDesignCompletion, QueryDesignRequest};

impl Desktop {
    pub(super) fn describe_query_design(&mut self, request: QueryDesignRequest, table: String) {
        if self.editing.has_save() || !self.state.can_describe_query_design(&request, &table) {
            return;
        }
        let Some(request_id) = request_read(
            Command::DescribeQueryDesign {
                target: native_target(&request),
                // NO_CLONE: Worker source metadata request owns table text while UI pending QBE
                // description and host witness retain the source independently.
                table: table.clone(),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        // NO_CLONE: The UI pending description retains request revision and table identity; the
        // host retains its own request/table witness to validate the native reply.
        if !self
            .state
            .begin_query_design_description(request.clone(), table.clone(), request_id)
        {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::QueryDesignDescribe {
            request_id,
            request,
            table,
        };
        self.state.busy = true;
    }

    pub(super) fn build_query_design(
        &mut self,
        request: QueryDesignRequest,
        draft: Arc<QueryDesignDraft>,
        completion: QueryDesignCompletion,
    ) {
        if self.editing.has_save()
            || !self
                .state
                .can_build_query_design(&request, &draft, completion)
        {
            return;
        }
        let Some(request_id) = request_read(
            Command::BuildQueryDesign {
                target: native_target(&request),
                // NO_CLONE: UI pending and the worker retain the same immutable submitted draft;
                // Arc shares its vectors/text instead of repeating the editor's deep
                // copy.
                draft: Arc::clone(&draft),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        // NO_CLONE: The shared pending QBE build and desktop pending operation each own the exact
        // object/publication/draft-revision witness.
        if !self
            .state
            .begin_query_design_build(request.clone(), draft, completion, request_id)
        {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::QueryDesignBuild {
            request_id,
            request,
        };
        self.state.busy = true;
    }

    pub(super) fn import_query_design(&mut self, request: QueryDesignRequest, sql: String) {
        if self.editing.has_save() || !self.state.can_import_query_design(&request, &sql) {
            return;
        }
        let Some(request_id) = request_read(
            Command::ImportQueryDesign {
                target: native_target(&request),
                // NO_CLONE: Runtime import owns SQL while UI pending import stores the exact
                // submitted SQL; unsupported imports must preserve the prior
                // draft/results.
                sql: sql.clone(),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        // NO_CLONE: Shared pending import and desktop pending reply validator each retain the exact
        // design request identity.
        if !self
            .state
            .begin_query_design_import(request.clone(), sql, request_id)
        {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::QueryDesignImport {
            request_id,
            request,
        };
        self.state.busy = true;
    }

    pub(super) fn cancel_query_design(&mut self, request_id: u64) {
        let owned = matches!(
            self.pending,
            PendingOperation::QueryDesignDescribe { request_id: expected, .. }
                | PendingOperation::QueryDesignBuild { request_id: expected, .. }
                | PendingOperation::QueryDesignImport { request_id: expected, .. }
                | PendingOperation::QueryDesignRun { request_id: expected }
                if expected == request_id
        );
        if self.state.cancel_query_design(request_id) && owned {
            self.worker.cancel();
            self.state.busy = false;
        }
    }

    pub(super) fn publish_query_design_table(
        &mut self,
        request_id: u64,
        target: runtime::QueryDesignTarget,
        metadata: DesignTableMetadata,
    ) {
        let PendingOperation::QueryDesignDescribe {
            request_id: expected,
            request,
            table,
        } = &self.pending
        else {
            return;
        };
        if *expected != request_id {
            return;
        }
        if target.document_revision != request.target.document_revision
            || metadata.table != *table
            || !self.state.set_query_design_table(request_id, metadata)
        {
            self.state
                .fail_query_design(request_id, self.state.localizer.text("qbe-invalid-result"));
        }
    }

    pub(super) fn publish_query_design_built(
        &mut self,
        request_id: u64,
        target: runtime::QueryDesignTarget,
        built: BuiltQueryDesign,
        metadata: Vec<DesignTableMetadata>,
    ) {
        let (expected, request) = match &self.pending {
            PendingOperation::QueryDesignBuild {
                request_id,
                request,
            }
            | PendingOperation::QueryDesignImport {
                request_id,
                request,
            } => (*request_id, request),
            _ => return,
        };
        if expected != request_id {
            return;
        }
        if target.document_revision != request.target.document_revision {
            self.state
                .fail_query_design(request_id, self.state.localizer.text("qbe-invalid-result"));
            return;
        }
        match self
            .state
            .set_query_design_built(request_id, built, metadata)
        {
            Some(QueryDesignCompletion::Run) => self.run_query_design(),
            Some(QueryDesignCompletion::ShowSql) => {}
            None => self
                .state
                .fail_query_design(request_id, self.state.localizer.text("qbe-invalid-result")),
        }
    }

    fn run_query_design(&mut self) {
        if self.editing.has_save() {
            return;
        }
        let Some(request_id) = request_read(
            Command::Query {
                // NO_CLONE: The UI keeps its SQL buffer and result provenance; background query
                // execution requires an independent owned command string.
                sql: self.state.sql.clone(),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_query_design_run(request_id) {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::QueryDesignRun { request_id };
        self.state.busy = true;
    }
}

fn native_target(request: &QueryDesignRequest) -> runtime::QueryDesignTarget {
    runtime::QueryDesignTarget {
        document_revision: request.target.document_revision,
    }
}

#[cfg(test)]
mod tests;
