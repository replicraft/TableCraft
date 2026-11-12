//! Closed ordinary/unique DROP plans retain their distinct native originals.
use tablecraft_ace::{RecoverableNativeIndexDropPlan, RecoverableNativeUniqueLongIndexDropPlan};

pub(in crate::editing) enum Plan {
    Original(RecoverableNativeIndexDropPlan),
    UniqueLong(RecoverableNativeUniqueLongIndexDropPlan),
}

pub(super) struct Target {
    pub native_table: i32,
    pub definition: u32,
    pub logical: u32,
    pub physical: u32,
}

impl Plan {
    pub(super) fn target(&self) -> Target {
        match self {
            Self::Original(plan) => {
                let intent = plan.intent();
                Target {
                    native_table: intent.native_table_id(),
                    definition: intent.definition_page(),
                    logical: intent.logical_index_number(),
                    physical: intent.physical_index_number(),
                }
            }
            Self::UniqueLong(plan) => {
                let intent = plan.intent();
                Target {
                    native_table: intent.native_table_id(),
                    definition: intent.definition_page(),
                    logical: intent.logical_index_number(),
                    physical: intent.physical_index_number(),
                }
            }
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::Original(plan) => plan.table(),
            Self::UniqueLong(plan) => plan.table(),
        }
    }

    pub(super) fn index_name(&self) -> &str {
        match self {
            Self::Original(plan) => plan.index_name(),
            Self::UniqueLong(plan) => plan.index_name(),
        }
    }

    pub(in crate::editing) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Original(plan) => plan.before_bytes(),
            Self::UniqueLong(plan) => plan.before_bytes(),
        }
    }

    pub(in crate::editing) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Original(plan) => plan.after_bytes(),
            Self::UniqueLong(plan) => plan.after_bytes(),
        }
    }

    pub(super) fn after_shared_bytes(&self) -> bytes::Bytes {
        match self {
            Self::Original(plan) => plan.after_shared_bytes(),
            Self::UniqueLong(plan) => plan.after_shared_bytes(),
        }
    }
}
