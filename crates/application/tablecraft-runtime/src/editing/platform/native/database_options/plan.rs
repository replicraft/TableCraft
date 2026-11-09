//! The selected sealed source purpose survives through the one save owner.
use tablecraft_ace::{
    RecoverableFieldHistoryDatabaseOptionsPlan, RecoverableNativeDatabaseOptionsPlan,
};

pub(in crate::editing) enum Plan {
    Creator(RecoverableNativeDatabaseOptionsPlan),
    FieldHistory(RecoverableFieldHistoryDatabaseOptionsPlan),
}

impl Plan {
    pub(in crate::editing) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Creator(plan) => plan.before_bytes(),
            Self::FieldHistory(plan) => plan.before_bytes(),
        }
    }

    pub(in crate::editing) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Creator(plan) => plan.after_bytes(),
            Self::FieldHistory(plan) => plan.after_bytes(),
        }
    }
}
