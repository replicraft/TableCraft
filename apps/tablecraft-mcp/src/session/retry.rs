//! Failed INSERT continuation checks original bytes and starts a fresh draft, never replay.
use super::{Session, arguments as arg};
use serde_json::{Value, json};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, InsertRetryEvidence, InsertRetryWitness, InsertTarget, SaveReport,
};

pub(super) struct InsertRetry {
    pub save_id: u64,
    pub witness: Arc<InsertRetryWitness>,
    pub evidence: InsertRetryEvidence,
}

impl Session {
    pub(super) fn observe_insert_retry(&mut self, report: SaveReport) {
        if let Some(retry) = self
            .insert_retry
            .as_mut()
            .filter(|retry| retry.save_id == report.id)
        {
            retry.evidence.observe(report);
        }
    }

    pub(super) fn retry_status(&self) -> Value {
        let Some(retry) = &self.insert_retry else {
            return Value::Null;
        };
        json!({
            "save_id": retry.save_id,
            "table": retry.witness.target().table,
            "original_retry_eligible": retry.evidence.can_retry_original(),
            "native_owners_released": self.active_save.is_none() && !self.saves.is_busy(),
        })
    }

    pub(super) fn insertion_retry_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        if self.active_save.is_some() || self.saves.is_busy() || self.creation.is_busy() {
            return Err(
                "Resolve and acknowledge the actual native owner before insertion continuation"
                    .into(),
            );
        }
        let requested = arg::u64_value(args, "save_id")?;
        let Some(retry) = self
            .insert_retry
            .as_ref()
            .filter(|retry| retry.save_id == requested)
        else {
            return Err("Unknown or superseded insertion retry context".into());
        };
        if operation == "discard_insert_retry" {
            self.insert_retry = None;
            return Ok(json!({"save_id": requested, "discarded": true, "native_write": false}));
        }
        if self.uncertain || !retry.evidence.can_retry_original() {
            return Err(
                "The actual save chain does not permit original-image insertion continuation"
                    .into(),
            );
        }
        // NO_CLONE: Keep the actual immutable original witness after a refused or cancelled
        // read; the worker independently owns the same exact image during verification.
        let witness = Arc::clone(&retry.witness);
        // NO_CLONE: BeginInsert requires owned table identity after the original-source read
        // crosses the worker boundary. The retained witness stays available for retry.
        let table = retry.witness.target().table.clone();
        let response = self.request(Command::OpenInsertOriginal { witness })?;
        let catalog = self.response(response)?;
        // Only the actual complete-original witness read resolves this released
        // image restriction. Native actor uncertainty is never cleared here.
        self.refresh_required = false;
        self.insert_retry = None;
        let revision = self
            .revision
            .ok_or("Original-image reopening did not publish a document revision")?;
        let response = self.request(Command::BeginInsert {
            target: InsertTarget {
                document_revision: revision,
                table,
            },
        })?;
        let draft = self.response(response)?;
        Ok(
            json!({"previous_save_id": requested, "original_verified": true, "native_write": false, "result": catalog, "draft": draft}),
        )
    }
}
