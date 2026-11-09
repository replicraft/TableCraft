//! One serialized local host session; JSON cannot construct native capabilities.
mod arguments;
mod auxiliary;
mod command;
mod creation;
mod filtering;
mod query_design;
mod response;
mod retry;
mod reviewed;
mod save;
mod unchanged_field_rename;

use reviewed::Reviewed;
use save::ActiveSave;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use tablecraft_runtime::{
    Command, CreationReport, CreationWorker, InspectedQuery, Response, SaveWorker, Worker,
};

const READ_POLL_INTERVAL: Duration = Duration::from_millis(2);

pub struct Session {
    worker: Worker,
    saves: SaveWorker,
    creation: CreationWorker,
    revision: Option<u64>,
    path: Option<PathBuf>,
    catalog: Value,
    query: Option<(u64, Arc<InspectedQuery>)>,
    reviewed: Option<(u64, Reviewed)>,
    active_save: Option<ActiveSave>,
    insert_retry: Option<retry::InsertRetry>,
    unchanged_field_rename: Option<unchanged_field_rename::PendingRename>,
    next_handle: u64,
    created: Option<Arc<CreationReport>>,
    drop_source: Option<tablecraft_runtime::TableDropSourceInspection>,
    uncertain: bool,
    refresh_required: bool,
    creation_id: Option<u64>,
    find_state: auxiliary::FindState,
    view_state: filtering::ViewState,
    design_state: query_design::DesignState,
    request_cancellation: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    read_dispatched: Option<Box<dyn FnOnce(u64) + Send>>,
}

impl Session {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            worker: Worker::new(|| {}),
            saves: SaveWorker::new(|| {}),
            creation: CreationWorker::new(|| {}),
            revision: None,
            path: None,
            catalog: Value::Null,
            query: None,
            reviewed: None,
            active_save: None,
            insert_retry: None,
            unchanged_field_rename: None,
            next_handle: 0,
            created: None,
            drop_source: None,
            uncertain: false,
            refresh_required: false,
            creation_id: None,
            find_state: auxiliary::FindState::default(),
            view_state: filtering::ViewState::default(),
            design_state: query_design::DesignState::default(),
            request_cancellation: None,
            #[cfg(test)]
            read_dispatched: None,
        })
    }

    /// Scope peer cancellation to this call; accepted native saves retain their owner.
    pub(super) fn execute_cancellable(
        &mut self,
        operation: &str,
        args: Value,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Value, String> {
        if cancelled.load(Ordering::Acquire) {
            return Err("Tool request was cancelled before execution".into());
        }
        let previous = self.request_cancellation.replace(cancelled);
        let result = self.execute(operation, args);
        self.request_cancellation = previous;
        result
    }

    pub fn execute(&mut self, operation: &str, args: Value) -> Result<Value, String> {
        match operation {
            "session_status" => Ok(json!({
                "document_revision": self.revision,
                "document_published": self.revision.is_some(),
                "unchanged_field_rename_operation_id": self.unchanged_field_rename.as_ref().map(|pending| pending.id),
                "active_save_id": self.active_save.as_ref().map(|save| save.id),
                "active_creation_id": self.creation_id.filter(|_| self.creation.is_busy()),
                "source_uncertain": self.uncertain || self.refresh_required,
                "native_save_uncertain": self.uncertain,
                "source_refresh_required": self.refresh_required,
                "catalog": &self.catalog,
                "insert_retry": self.retry_status(),
            })),
            "retry_unchanged_field_rename" | "discard_unchanged_field_rename" => {
                self.unchanged_field_rename_operation(operation, &args)
            }
            "retry_insert" | "discard_insert_retry" => {
                self.insertion_retry_operation(operation, &args)
            }
            "start_reserved_recovery" => self.start_reserved_recovery(&args),
            "list_tables" => Ok(json!({"catalog": &self.catalog})),
            "save_status" | "cancel_save" | "recover_save" | "release_save"
            | "acknowledge_save" => self.save_operation(operation, &args),
            "create_database"
            | "creation_status"
            | "cancel_creation"
            | "acknowledge_creation"
            | "open_created_database" => self.creation_operation(operation, &args),
            _ if operation.starts_with("apply_") && operation != "apply_field_filter" => {
                self.apply_review(operation, &args)
            }
            _ => {
                if self.active_save.is_some()
                    || self.saves.is_busy()
                    || self.creation.is_busy()
                    || self.unchanged_field_rename.is_some()
                {
                    return Err("Resolve and acknowledge the active save or creation before issuing document requests".into());
                }
                if self.uncertain {
                    return Err("The native save result is uncertain; restart this session and inspect recovery before reopening".into());
                }
                if self.refresh_required {
                    return Err("The released source image needs exact verification; use an eligible original-image insertion continuation or restart this session".into());
                }
                if matches!(operation, "find_next" | "continue_find" | "replace_found") {
                    return self.find_operation(operation, &args);
                }
                if matches!(
                    operation,
                    "describe_field_filter"
                        | "apply_field_filter"
                        | "filter_selection"
                        | "filter_page"
                        | "clear_filter"
                        | "reapply_filter"
                        | "sort_view"
                        | "describe_totals"
                        | "calculate_totals"
                ) {
                    return self.view_operation(operation, &args);
                }
                if matches!(
                    operation,
                    "describe_query_design"
                        | "import_query_design"
                        | "build_query_design"
                        | "rebuild_query_design"
                ) {
                    return self.design_operation(operation, &args);
                }
                let command = self.command(operation, &args)?;
                let response = self.request(command)?;
                self.response(response)
            }
        }
    }

    fn current_revision(&self, args: &Value) -> Result<u64, String> {
        let revision = self.revision.ok_or("Open a database first")?;
        if arguments::u64_value(args, "document_revision")? != revision {
            return Err("The supplied document revision is stale".into());
        }
        Ok(revision)
    }

    fn next_handle(&mut self) -> Result<u64, String> {
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .ok_or("Session handle identities exhausted")?;
        Ok(self.next_handle)
    }

    fn read_cancelled(&self) -> bool {
        self.request_cancellation
            .as_ref()
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
    }

    fn retire_cancelled_read_publication(&mut self) {
        // An Open can install its new runtime source before cancellation revokes
        // its reply. Retire published authority rather than reusing old revisions
        // against an unpublished source. Native save/creation owners are separate.
        self.revision = None;
        self.path = None;
        self.catalog = Value::Null;
        self.query = None;
        self.reviewed = None;
        self.drop_source = None;
        self.find_state = auxiliary::FindState::default();
        self.view_state = filtering::ViewState::default();
        self.design_state = query_design::DesignState::default();
    }

    fn request(&mut self, command: Command) -> Result<Response, String> {
        if self.read_cancelled() {
            return Err("Read request was cancelled before dispatch".into());
        }
        let id = self
            .worker
            .request(command)
            .map_err(|error| error.to_string())?;
        #[cfg(test)]
        if let Some(dispatched) = self.read_dispatched.take() {
            dispatched(id);
        }
        loop {
            if self.read_cancelled() {
                self.worker.cancel();
                self.retire_cancelled_read_publication();
                return Err("Read cancellation requested; reopen the database before issuing further document requests. Accepted native saves and creation outcomes are unchanged".into());
            }
            if let Some((actual, response)) = self.worker.poll() {
                if actual != id {
                    return Err("A different request owns the runtime publication".into());
                }
                return response.map_err(|error| error.to_string());
            }
            thread::sleep(READ_POLL_INTERVAL);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cancellation_tests;

#[cfg(test)]
mod acceptance;
