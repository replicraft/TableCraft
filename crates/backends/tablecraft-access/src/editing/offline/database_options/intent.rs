//! Exact version selection before decoding; no failed decoder is retried.
use super::super::Cause;
use tablecraft_ace::{NativeDatabaseOptionsIntent, NativeFieldHistoryDatabaseOptionsIntent};
use tablecraft_core::limits::ResourceBudget;

const FIELD_HISTORY_MAGIC: &[u8; 8] = b"TCDO0002";

pub(in crate::editing::offline) enum Intent {
    Creator(NativeDatabaseOptionsIntent),
    FieldHistory(NativeFieldHistoryDatabaseOptionsIntent),
}

impl Intent {
    pub(in crate::editing::offline) fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Cause> {
        if raw.starts_with(FIELD_HISTORY_MAGIC) {
            return NativeFieldHistoryDatabaseOptionsIntent::decode(raw, budget, cancelled)
                .map(Self::FieldHistory)
                .map_err(Cause::FieldHistoryDatabaseOptionsIntent);
        }
        // Retain the original decoder's exact malformed/unknown-version errors.
        // This branch is chosen before decoding and never follows an error.
        NativeDatabaseOptionsIntent::decode(raw, budget, cancelled)
            .map(Self::Creator)
            .map_err(Cause::DatabaseOptionsIntent)
    }
}
