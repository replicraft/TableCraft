//! Retained actors, current outcomes and exact expected-image reopening stay distinct.
use super::{Session, arguments as arg};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tablecraft_runtime::{
    AddedColumnReopen, Command, CreatedTableReopen, DatabaseOptionsReopen, FieldDroppedReopen,
    FieldRenamedReopen, QueryExpressionReopen, SaveKnownOutcome, SaveOutcome, SaveReceipt,
    SaveReport, TableDroppedReopen, TableRenameReopen,
};

pub(super) struct ActiveSave {
    pub id: u64,
    pub path: PathBuf,
    pub witness: Option<Reopen>,
    pub acknowledged: bool,
    pub report: Option<SaveReport>,
    pub restart: bool,
}

pub(super) enum Reopen {
    TableRename(Arc<TableRenameReopen>),
    TableDrop(Arc<TableDroppedReopen>),
    TableCreation(Arc<CreatedTableReopen>),
    FieldAddition(Arc<AddedColumnReopen>),
    FieldRename(Arc<FieldRenamedReopen>),
    FieldDrop(Arc<FieldDroppedReopen>),
    QueryExpression(Arc<QueryExpressionReopen>),
    DatabaseOptions(Arc<DatabaseOptionsReopen>),
}

impl Reopen {
    fn replacement(&self, outcome: SaveOutcome) -> Result<bool, String> {
        match self {
            Self::TableDrop(_) => outcome
                .table_drop_reopen_facts()
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
            Self::FieldAddition(_) => outcome
                .field_addition_reopen_facts()
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
            Self::FieldRename(_) => outcome
                .field_rename_reopen_facts()
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
            Self::FieldDrop(_) => outcome
                .field_drop_reopen_facts()
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
            Self::QueryExpression(_) => outcome
                .query_expression_reopen_facts()
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
            Self::DatabaseOptions(_) => outcome
                .database_options_reopen_facts()
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
            Self::TableRename(_) => outcome
                .table_rename_reopen_facts()
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
            Self::TableCreation(witness) => outcome
                .table_creation_reopen_facts(witness)
                .map(|facts| facts.replacement)
                .map_err(|error| error.to_string()),
        }
    }

    fn command(&self) -> Command {
        match self {
            // NO_CLONE: Retain the actual complete DROP image for retry while its read
            // command independently owns the same immutable witness across threads.
            Self::TableDrop(witness) => Command::OpenTableDropped {
                witness: Arc::clone(witness),
            },
            // NO_CLONE: The host retains the same immutable complete-image witness for retry;
            // this worker command needs a shared reference across its thread boundary.
            Self::TableRename(witness) => Command::OpenTableRenamed {
                witness: Arc::clone(witness),
            },
            // NO_CLONE: Keep expected bytes owned during a failed read so an explicit retry
            // can use this same witness rather than an unchecked ordinary path open.
            Self::TableCreation(witness) => Command::OpenTableCreated {
                witness: Arc::clone(witness),
            },
            // NO_CLONE: Actor and host independently retain this immutable witness through
            // full-image verification and any explicitly requested reopening retry.
            Self::FieldAddition(witness) => Command::OpenColumnAdded {
                witness: Arc::clone(witness),
            },
            // NO_CLONE: The worker reads the immutable expected image while this host retains
            // its exact owner for retry; a frame/call borrow cannot cross the actor boundary.
            Self::FieldRename(witness) => Command::OpenFieldRenamed {
                witness: Arc::clone(witness),
            },
            // NO_CLONE: This complete-image owner must survive a failed DROP reopen and is
            // shared, not copied, into the worker's independently owned command.
            Self::FieldDrop(witness) => Command::OpenFieldDropped {
                witness: Arc::clone(witness),
            },
            // NO_CLONE: Retain the actual query-only expected-image owner across asynchronous
            // verification without copying its database bytes or inventing a new authority.
            Self::QueryExpression(witness) => Command::OpenQueryExpressionSaved {
                witness: Arc::clone(witness),
            },
            // NO_CLONE: Settings reopening uses the same immutable expected image while the
            // host keeps it available if the bounded read fails and needs an explicit retry.
            Self::DatabaseOptions(witness) => Command::OpenDatabaseOptionsSaved {
                witness: Arc::clone(witness),
            },
        }
    }
}

fn known_replacement(known: SaveKnownOutcome) -> bool {
    matches!(
        known,
        SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified
    )
}

fn replacement_receipt(outcome: SaveOutcome) -> Result<Option<SaveReceipt>, String> {
    match outcome {
        SaveOutcome::Uncertain { .. } => {
            Err("The save actor ended without a native terminal result".into())
        }
        SaveOutcome::Saved { receipt } => Ok(known_replacement(receipt.known).then_some(receipt)),
        SaveOutcome::Released {
            current_outcome,
            last_completed,
        } => match current_outcome {
            Some(known) if !known_replacement(known) => Ok(None),
            Some(_) => last_completed
                .filter(|receipt| known_replacement(receipt.known))
                .map(Some)
                .ok_or("Known replacement is missing its operation receipt".into()),
            None => Ok(last_completed.filter(|receipt| known_replacement(receipt.known))),
        },
        SaveOutcome::Failed(failure) => {
            if failure.can_recover || failure.can_release || failure.state.is_some() {
                return Err("The save still owns native recovery resources".into());
            }
            match failure.current_outcome {
                Some(known) if !known_replacement(known) => Ok(None),
                Some(_) => failure
                    .last_completed
                    .filter(|receipt| known_replacement(receipt.known))
                    .map(Some)
                    .ok_or("Known replacement is missing its operation receipt".into()),
                None if failure.last_completed.is_some() => Ok(failure
                    .last_completed
                    .filter(|receipt| known_replacement(receipt.known))),
                None => Ok(None),
            }
        }
    }
}

pub(super) fn report_json(report: SaveReport) -> Value {
    let outcome = match report.outcome {
        SaveOutcome::Saved { receipt } => {
            json!({"kind": "saved", "receipt": receipt_json(receipt)})
        }
        SaveOutcome::Released {
            current_outcome,
            last_completed,
        } => {
            json!({"kind": "released", "current_outcome": current_outcome.map(|known| format!("{known:?}")), "last_completed": last_completed.map(receipt_json)})
        }
        SaveOutcome::Uncertain {
            last_known_outcome,
            last_completed,
        } => {
            json!({"kind": "uncertain", "last_known_outcome": last_known_outcome.map(|known| format!("{known:?}")), "last_completed": last_completed.map(receipt_json), "retry_allowed": false})
        }
        SaveOutcome::Failed(failure) => {
            json!({"kind": "failed", "reason": format!("{:?}", failure.kind), "phase": failure.phase.map(|phase| format!("{phase:?}")), "state": failure.state.map(|state| format!("{state:?}")), "current_outcome": failure.current_outcome.map(|known| format!("{known:?}")), "last_completed": failure.last_completed.map(receipt_json), "can_recover": failure.can_recover, "can_release": failure.can_release})
        }
    };
    json!({"save_id": report.id, "operation": format!("{:?}", report.operation), "status": "terminal", "outcome": outcome})
}

fn receipt_json(receipt: SaveReceipt) -> Value {
    json!({"completion": format!("{:?}", receipt.completion), "known": format!("{:?}", receipt.known), "matched_rows": receipt.matched_rows, "changed_rows": receipt.changed_rows, "schema_change": receipt.schema_change.map(|change| format!("{change:?}")), "query_change": receipt.query_change.map(|change| format!("{change:?}")), "database_options_change": receipt.database_options_change.map(|change| format!("{change:?}")), "retirement": receipt.retirement.map(|guarantee| format!("{guarantee:?}")), "cleanup": receipt.cleanup.map(|cleanup| format!("{cleanup:?}"))})
}

impl Session {
    pub(super) fn start_reserved_recovery(&mut self, args: &Value) -> Result<Value, String> {
        if self.unchanged_field_rename.is_some()
            || self.active_save.is_some()
            || self.saves.is_busy()
            || self.creation.is_busy()
            || self.uncertain
            || self.refresh_required
        {
            return Err("An existing native owner blocks restart recovery".into());
        }
        let path = PathBuf::from(arg::text(args, "path")?);
        if !path.is_absolute() {
            return Err("Recovery source path must be absolute".into());
        }
        let id = self
            .saves
            .start_reserved_recovery(path.to_path_buf())
            .map_err(|error| error.to_string())?;
        self.active_save = Some(ActiveSave {
            id,
            path,
            witness: None,
            acknowledged: false,
            report: None,
            restart: true,
        });
        self.reviewed = None;
        self.insert_retry = None;
        self.query = None;
        self.find_state = super::auxiliary::FindState::default();
        self.view_state.retire_native();
        self.design_state = super::query_design::DesignState::default();
        Ok(json!({"save_id": id, "status": "pending"}))
    }

    pub(super) fn save_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let id = arg::u64_value(args, "save_id")?;
        let Some(active) = self.active_save.as_ref().filter(|active| active.id == id) else {
            return Err("Unknown or superseded save handle".into());
        };
        let acknowledged = active.acknowledged;
        let report = active
            .report
            .or_else(|| self.saves.poll().filter(|report| report.id == id));
        if let Some(report) = report {
            self.observe_insert_retry(report);
        }
        match operation {
            "save_status" => {
                if let Some(report) = report {
                    if matches!(report.outcome, SaveOutcome::Uncertain { .. }) {
                        self.uncertain = true;
                    }
                    return Ok(report_json(report));
                }
                Ok(json!({"save_id": id, "status": "pending"}))
            }
            "cancel_save" => {
                Ok(json!({"save_id": id, "cancellation_requested": self.saves.cancel(id)}))
            }
            "recover_save" | "release_save" => {
                if acknowledged {
                    return Err("This actor is already acknowledged; only its exact reopening may be retried".into());
                }
                let next = if operation == "recover_save" {
                    self.saves.recover(id)
                } else {
                    self.saves.release(id)
                }
                .map_err(|error| error.to_string())?;
                let Some(active) = self.active_save.as_mut() else {
                    return Err("The native save owner was lost".into());
                };
                active.id = next;
                active.report = None;
                if let Some(retry) = self
                    .insert_retry
                    .as_mut()
                    .filter(|retry| retry.save_id == id)
                {
                    retry.save_id = next;
                }
                Ok(json!({"save_id": next, "previous_save_id": id, "status": "pending"}))
            }
            "acknowledge_save" => {
                self.acknowledge_save(id, report.ok_or("The save has no terminal report yet")?)
            }
            _ => Err("Unknown save operation".into()),
        }
    }

    fn acknowledge_save(&mut self, id: u64, report: SaveReport) -> Result<Value, String> {
        let active = self
            .active_save
            .as_ref()
            .ok_or("The native save owner was lost")?;
        if matches!(report.outcome, SaveOutcome::Uncertain { .. }) {
            if !self.saves.acknowledge(id) {
                return Err("The uncertain actor could not be acknowledged".into());
            }
            self.uncertain = true;
            self.active_save = None;
            return Ok(
                json!({"save_id": id, "acknowledged": true, "source_uncertain": true, "reopened": false}),
            );
        }
        let replacement = match &active.witness {
            Some(witness) => witness.replacement(report.outcome)?,
            None => replacement_receipt(report.outcome)?.is_some(),
        };
        if !active.acknowledged && !self.saves.acknowledge(id) {
            return Err("The save still owns resources or another terminal identity".into());
        }
        let Some(active) = self.active_save.as_mut() else {
            return Err("The native save owner was lost".into());
        };
        active.acknowledged = true;
        active.report = Some(report);
        let current_original = match report.outcome {
            SaveOutcome::Saved { receipt } => !known_replacement(receipt.known),
            SaveOutcome::Released {
                current_outcome, ..
            } => current_outcome.is_some_and(|known| !known_replacement(known)),
            SaveOutcome::Failed(failure) => failure
                .current_outcome
                .is_some_and(|known| !known_replacement(known)),
            SaveOutcome::Uncertain { .. } => false,
        };
        if !replacement && !current_original && !report.stopped_before_target_writes() {
            self.refresh_required = true;
            self.active_save = None;
            return Ok(
                json!({"save_id": id, "acknowledged": true, "reopened": false, "replacement": false, "source_verified": false, "source_uncertain": true, "native_save_uncertain": self.uncertain, "source_refresh_required": true, "insert_retry": self.retry_status()}),
            );
        }
        if !replacement && !active.restart {
            self.active_save = None;
            return Ok(
                json!({"save_id": id, "acknowledged": true, "reopened": false, "replacement": false, "insert_retry": self.retry_status()}),
            );
        }
        let command = match &active.witness {
            Some(witness) => witness.command(),
            None => Command::Open {
                path: active.path.to_path_buf(),
                password: String::new(),
            },
        };
        let response = self.request(command)?;
        let catalog = self.response(response)?;
        self.active_save = None;
        self.insert_retry = None;
        Ok(
            json!({"save_id": id, "acknowledged": true, "reopened": true, "replacement": replacement, "source_verified": true, "result": catalog}),
        )
    }
}
