//! Private original/fresh composition, chosen before native consumption.
use tablecraft_ace::{
    RecoverableNativeFreshIndexCreatePlan, RecoverableNativeIndexCreatePlan,
    RecoverableNativeNumericIndexCreatePlan,
};

pub(in crate::editing) enum Plan {
    Original(RecoverableNativeIndexCreatePlan),
    Fresh(RecoverableNativeFreshIndexCreatePlan),
    Numeric(RecoverableNativeNumericIndexCreatePlan),
}

/// Actual native field witness copied into the immutable guarded-reopen owner.
pub(super) struct Target {
    pub native_table: i32,
    pub definition: u32,
    pub number: u16,
    pub column_id: u16,
    pub descending: bool,
}

impl Plan {
    pub(super) fn target(&self) -> Target {
        match self {
            Self::Original(plan) => {
                let intent = plan.intent();
                Target {
                    native_table: intent.native_table_id(),
                    definition: intent.definition_page(),
                    number: intent.native_number(),
                    column_id: intent.column_id(),
                    descending: intent.descending(),
                }
            }
            Self::Fresh(plan) => {
                let intent = plan.intent();
                Target {
                    native_table: intent.native_table_id(),
                    definition: intent.definition_page(),
                    number: intent.native_number(),
                    column_id: intent.column_id(),
                    descending: intent.descending(),
                }
            }
            Self::Numeric(plan) => {
                let intent = plan.intent();
                Target {
                    native_table: intent.native_table_id(),
                    definition: intent.definition_page(),
                    number: intent.native_number(),
                    column_id: intent.column_id(),
                    descending: intent.descending(),
                }
            }
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::Original(plan) => plan.table(),
            Self::Fresh(plan) => plan.table(),
            Self::Numeric(plan) => plan.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::Original(plan) => plan.column(),
            Self::Fresh(plan) => plan.column(),
            Self::Numeric(plan) => plan.column(),
        }
    }

    pub(super) fn index_name(&self) -> &str {
        match self {
            Self::Original(plan) => plan.index_name(),
            Self::Fresh(plan) => plan.index_name(),
            Self::Numeric(plan) => plan.index_name(),
        }
    }

    pub(super) fn indexed_rows(&self) -> usize {
        match self {
            Self::Original(plan) => plan.indexed_rows(),
            Self::Fresh(plan) => plan.indexed_rows(),
            Self::Numeric(plan) => plan.indexed_rows(),
        }
    }

    pub(in crate::editing) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Original(plan) => plan.before_bytes(),
            Self::Fresh(plan) => plan.before_bytes(),
            Self::Numeric(plan) => plan.before_bytes(),
        }
    }

    pub(in crate::editing) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Original(plan) => plan.after_bytes(),
            Self::Fresh(plan) => plan.after_bytes(),
            Self::Numeric(plan) => plan.after_bytes(),
        }
    }

    pub(super) fn after_shared_bytes(&self) -> bytes::Bytes {
        match self {
            Self::Original(plan) => plan.after_shared_bytes(),
            Self::Fresh(plan) => plan.after_shared_bytes(),
            Self::Numeric(plan) => plan.after_shared_bytes(),
        }
    }
}
