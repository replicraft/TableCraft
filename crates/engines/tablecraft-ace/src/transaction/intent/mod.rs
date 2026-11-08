//! Original typed requests bound to one before image, never serialized save permission.
mod binding;
mod codec;
mod dispatch;
mod fitted;
mod fixed;
mod model;
mod planning;

pub use dispatch::{NativeUpdateBody, NativeUpdateIntent, NativeUpdateProfile};
pub use fitted::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};
pub use fixed::{NativeFixedEditIntent, NativeFixedIntentGroup};
pub use model::{NativeInt32EditIntent, NativeIntentError};
pub use planning::{RecoverableEditError, RecoverableNativeEditPlan};

#[cfg(test)]
mod tests;

mod delete;
pub use delete::{
    NativeDeleteIntent, QualifiedNativeDeleteRecovery, RecoverableNativeDeletePlan,
    qualify_native_delete_recovery,
};

mod insert;
mod insert_requalification;
pub use insert::{
    NativeInsertIntent, QualifiedNativeInsertRecovery, RecoverableNativeInsertPlan,
    qualify_native_insert_recovery,
};

mod omitted_insert;
pub use omitted_insert::{
    NativeOmittedInsertIntent, QualifiedNativeOmittedInsertRecovery,
    RecoverableNativeOmittedInsertPlan, qualify_native_omitted_insert_recovery,
};

mod complete_insert;
mod eof_insert;
mod row_carrier;
pub use eof_insert::{
    NativeEofInsertIntent, QualifiedNativeEofInsertRecovery, RecoverableNativeEofInsertPlan,
    RecoverableNativeInsertOperation, qualify_native_eof_insert_recovery,
};

mod complete;
pub use complete::{
    NativeCompleteInsertIntent, QualifiedNativeCompleteInsertRecovery,
    RecoverableNativeCompleteInsertPlan, qualify_native_complete_insert_recovery,
};

mod clock_insert;
pub use clock_insert::{
    NativeClockInsertIntent, QualifiedNativeClockInsertRecovery, RecoverableNativeClockInsertPlan,
    qualify_native_clock_insert_recovery,
};

#[cfg(feature = "ace16-create")]
mod first_table;
#[cfg(feature = "ace16-create")]
pub use first_table::{
    FirstTableIntentError, NativeFirstTableIntent, PreparedFirstTable, QualifiedFirstTableRecovery,
    qualify_native_first_table_recovery,
};

#[cfg(feature = "ace16-create")]
mod add_table;
#[cfg(feature = "ace16-create")]
pub use add_table::{
    AddTableIntentError, NativeAddTableIntent, PreparedAddTable, QualifiedAddTableRecovery,
    qualify_native_add_table_recovery,
};

mod rename_table;
pub use rename_table::{
    NativeTableRenameIntent, PreparedTableRename, QualifiedTableRenameRecovery,
    RecoverableNativeTableRenamePlan, RecoverableTableRenameError, TableRenameIntentError,
    qualify_native_table_rename_recovery,
};
