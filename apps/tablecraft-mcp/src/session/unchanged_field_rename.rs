//! A consumed unchanged review retains only its exact read-only reopen witness.
use super::{Session, arguments as arg, reviewed::Reviewed};
use serde_json::{Value, json};
use std::sync::Arc;
use tablecraft_runtime::{Command, FieldRenamedReopen, ReviewedFieldRename};

pub(super) struct PendingRename {
    pub id: u64,
    witness: Arc<FieldRenamedReopen>,
}

impl Session {
    pub(super) fn accept_unchanged_field_rename(
        &mut self,
        id: u64,
        capability: ReviewedFieldRename,
    ) -> Result<Value, String> {
        let witness = match capability.into_unchanged_reopen() {
            Ok(witness) => witness,
            Err(capability) => {
                self.reviewed = Some((id, Reviewed::FieldRename(capability)));
                return Err("The unchanged field rename review is no longer valid".into());
            }
        };
        self.unchanged_field_rename = Some(PendingRename { id, witness });
        self.reopen_unchanged_field_rename()
    }

    pub(super) fn unchanged_field_rename_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let requested = arg::u64_value(args, "operation_id")?;
        if !self
            .unchanged_field_rename
            .as_ref()
            .is_some_and(|pending| pending.id == requested)
        {
            return Err("Unknown or completed unchanged field rename operation".into());
        }
        if operation == "discard_unchanged_field_rename" {
            self.unchanged_field_rename = None;
            self.retire_cancelled_read_publication();
            return Ok(json!({
                "operation_id": requested,
                "discarded": true,
                "native_write": false,
                "reopen_required": true
            }));
        }
        self.reopen_unchanged_field_rename()
    }

    fn reopen_unchanged_field_rename(&mut self) -> Result<Value, String> {
        let pending = self
            .unchanged_field_rename
            .as_ref()
            .ok_or("No accepted unchanged field rename remains")?;
        let id = pending.id;
        // NO_CLONE: The accepted operation retains its exact immutable image witness
        // across failed or cancelled reads; the runtime worker owns a shared reference.
        let witness = Arc::clone(&pending.witness);
        let response = self.request(Command::OpenFieldRenamed { witness })?;
        let catalog = self.response(response)?;
        self.unchanged_field_rename = None;
        Ok(json!({
            "operation_id": id,
            "changed": false,
            "native_write": false,
            "verified_reopen": true,
            "document": catalog
        }))
    }
}
