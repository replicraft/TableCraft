//! Blank publication is a separate retained actor, never an ordinary file write.
use super::{Session, arguments as arg};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tablecraft_runtime::{Command, CreationOutcome, CreationRequest};

impl Session {
    pub(super) fn creation_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        if operation == "create_database" {
            if self.active_save.is_some()
                || self.saves.is_busy()
                || self.creation.is_busy()
                || self.uncertain
                || self.refresh_required
            {
                return Err("Resolve the active operation before creating another database".into());
            }
            let destination = PathBuf::from(arg::text(args, "path")?);
            if !destination.is_absolute() {
                return Err("Creation path must be absolute".into());
            }
            let id = self
                .creation
                .start(CreationRequest::blank(destination))
                .map_err(|error| error.to_string())?;
            self.created = None;
            self.insert_retry = None;
            self.creation_id = Some(id);
            self.find_state = super::auxiliary::FindState::default();
            self.view_state.retire_native();
            self.design_state = super::query_design::DesignState::default();
            return Ok(json!({"creation_id": id, "status": "pending"}));
        }
        let requested = arg::u64_value(args, "creation_id")?;
        if self.creation_id != Some(requested) {
            return Err("Unknown or superseded creation handle".into());
        }
        match operation {
            "cancel_creation" => Ok(
                json!({"creation_id": requested, "cancellation_requested": self.creation.cancel(requested)}),
            ),
            "creation_status" => {
                let Some(report) = self.creation.poll() else {
                    if self.creation.is_busy() {
                        return Ok(json!({"creation_id": requested, "status": "pending"}));
                    }
                    return Err("No retained creation report exists".into());
                };
                if report.id != requested {
                    return Err("Unknown creation handle".into());
                }
                let result = match report.outcome() {
                    CreationOutcome::Created(receipt) => {
                        json!({"creation_id": report.id, "status": "created", "table_count": receipt.table_count(), "column_count": receipt.column_count(), "row_count": receipt.row_count(), "guarantee": format!("{:?}", receipt.guarantee()), "namespace_synced": receipt.namespace_synced()})
                    }
                    CreationOutcome::Failed(failure) => {
                        json!({"creation_id": report.id, "status": "failed", "phase": failure.phase.map(|phase| format!("{phase:?}")), "publication": format!("{:?}", failure.publication), "error": failure.to_string(), "automatic_retry": false})
                    }
                };
                self.created = Some(report);
                Ok(result)
            }
            "acknowledge_creation" => {
                let report = self
                    .creation
                    .poll()
                    .filter(|report| report.id == requested)
                    .ok_or("No matching terminal creation report")?;
                if !self.creation.acknowledge(requested) {
                    return Err("Creation still owns its operation or another report".into());
                }
                self.created = Some(report);
                Ok(json!({"creation_id": requested, "acknowledged": true}))
            }
            "open_created_database" => {
                if self.active_save.is_some() || self.saves.is_busy() || self.creation.is_busy() {
                    return Err("Acknowledge all native owners before opening created bytes".into());
                }
                let report = self
                    .created
                    .as_ref()
                    .filter(|report| report.id == requested)
                    .ok_or("No matching acknowledged creation report")?;
                if !matches!(report.outcome(), CreationOutcome::Created(_)) {
                    return Err("A failed publication does not authorize guarded opening".into());
                }
                // NO_CLONE: The host retains the same publication report for retry if its
                // guarded read fails; the worker independently owns only a shared reference.
                let report = Arc::clone(report);
                let response = self.request(Command::OpenCreated { report })?;
                self.response(response)
            }
            _ => Err("Unknown creation operation".into()),
        }
    }
}
