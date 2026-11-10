//! Original typed requests bound to one before image, never serialized save permission.
mod binding;
mod codec;
mod dispatch;
mod fitted;
mod fixed;
mod model;
mod planning;
mod row_ordinals;

pub use dispatch::{NativeUpdateBody, NativeUpdateIntent, NativeUpdateProfile};
pub use fitted::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};
pub use fixed::{NativeFixedEditIntent, NativeFixedIntentGroup};
pub use model::{NativeInt32EditIntent, NativeIntentError};
pub use planning::{RecoverableEditError, RecoverableNativeEditPlan};

#[cfg(test)]
mod tests;

mod delete;
pub use delete::{
    NativeDeleteIntent, QualifiedNativeDeleteRecovery, QualifiedRetainedNativeDeleteRecovery,
    RecoverableNativeDeletePlan, qualify_native_delete_recovery,
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
    QualifiedRetainedNativeOmittedInsertRecovery, RecoverableNativeOmittedInsertPlan,
    qualify_native_omitted_insert_recovery,
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

mod table_drop;
pub use table_drop::{
    NativeTableDropIntent, QualifiedRetainedTableDropRecovery, QualifiedTableDropRecovery,
    RecoverableTableDropPlan, TableDropIntentError, qualify_native_table_drop_recovery,
};

#[cfg(feature = "ace16-create")]
mod database_options_field_history;
#[cfg(feature = "ace16-create")]
pub use database_options_field_history::{
    FieldHistoryOptionsIntentError, NativeFieldHistoryDatabaseOptionsIntent,
    QualifiedFieldHistoryDatabaseOptionsRecovery, RecoverableFieldHistoryDatabaseOptionsPlan,
    qualify_native_field_history_database_options_recovery,
};

#[cfg(feature = "ace16-create")]
mod database_options;
#[cfg(feature = "ace16-create")]
pub use database_options::{
    DatabaseOptionsIntentError, NativeDatabaseOptionsIntent, QualifiedDatabaseOptionsRecovery,
    RecoverableNativeDatabaseOptionsPlan, qualify_native_database_options_recovery,
};

#[cfg(feature = "ace16-create")]
mod add_column;
#[cfg(feature = "ace16-create")]
pub use add_column::{
    AddColumnIntentError, NativeAddColumnIntent, QualifiedAddColumnRecovery,
    RecoverableAddColumnPlan, qualify_native_add_column_recovery,
};

#[cfg(feature = "ace16-create")]
mod default_zero_add_column;
#[cfg(feature = "ace16-create")]
pub use default_zero_add_column::{
    DefaultZeroAddColumnIntentError, NativeDefaultZeroAddColumnIntent,
    QualifiedDefaultZeroAddColumnRecovery, RecoverableDefaultZeroAddColumnPlan,
    qualify_native_default_zero_add_column_recovery,
};

#[cfg(feature = "ace16-create")]
mod repeated_default_zero_add_column;
#[cfg(feature = "ace16-create")]
pub use repeated_default_zero_add_column::{
    NativeRepeatedDefaultZeroAddColumnIntent, QualifiedRepeatedDefaultZeroAddColumnRecovery,
    RecoverableRepeatedDefaultZeroAddColumnPlan, RepeatedDefaultZeroAddColumnIntentError,
    qualify_native_repeated_default_zero_add_column_recovery,
};

mod query_expression_v2;
pub use query_expression_v2::{
    NativeQueryExpressionEditsIntent, PreparedQueryExpressionEdits,
    QualifiedQueryExpressionEditsRecovery, QueryExpressionEditsIntentError,
    RecoverableNativeQueryExpressionEditsPlan, qualify_native_query_expression_edits_recovery,
};

mod query_expression;
pub use query_expression::{
    NativeQueryExpressionIntent, PreparedQueryExpression, QualifiedQueryExpressionRecovery,
    QueryExpressionIntentError, QueryExpressionRecoveryError, RecoverableNativeQueryExpressionPlan,
    RecoverableQueryExpressionError, qualify_native_query_expression_recovery,
};

#[cfg(feature = "ace16-create")]
mod field_addition;
#[cfg(feature = "ace16-create")]
pub use field_addition::{
    FieldAdditionIntentError, NativeFieldAdditionIntent, QualifiedFieldAdditionRecovery,
    RecoverableFieldAdditionPlan, qualify_native_field_addition_recovery,
};

#[cfg(feature = "ace16-create")]
mod field_rename;
#[cfg(feature = "ace16-create")]
pub use field_rename::{
    FieldRenameIntentError, NativeFieldRenameIntent, QualifiedFieldRenameRecovery,
    RecoverableFieldRenamePlan, qualify_native_field_rename_recovery,
};

#[cfg(feature = "ace16-create")]
mod field_addition_after_off;
#[cfg(feature = "ace16-create")]
pub use field_addition_after_off::{
    FieldAdditionAfterOffIntentError, NativeFieldAdditionAfterOffIntent,
    QualifiedFieldAdditionAfterOffRecovery, RecoverableFieldAdditionAfterOffPlan,
    qualify_native_field_addition_after_off_recovery,
};

#[cfg(feature = "ace16-create")]
mod field_addition_after_off_history;
#[cfg(feature = "ace16-create")]
pub use field_addition_after_off_history::{
    FieldAdditionAfterOffHistoryIntentError, NativeFieldAdditionAfterOffHistoryIntent,
    QualifiedFieldAdditionAfterOffHistoryRecovery, RecoverableFieldAdditionAfterOffHistoryPlan,
    qualify_native_field_addition_after_off_history_recovery,
};

#[cfg(feature = "ace16-create")]
mod current_field_addition;
#[cfg(feature = "ace16-create")]
pub use current_field_addition::{
    CurrentFieldAdditionIntentError, NativeCurrentFieldAdditionIntent,
    QualifiedCurrentFieldAdditionRecovery, QualifiedRetainedFieldAdditionRecovery,
    RecoverableCurrentFieldAdditionPlan, qualify_native_current_field_addition_recovery,
};

#[cfg(feature = "ace16-create")]
mod field_drop;
#[cfg(feature = "ace16-create")]
pub use field_drop::{
    FieldDropIntentError, NativeFieldDropIntent, QualifiedFieldDropRecovery,
    RecoverableFieldDropPlan, qualify_native_field_drop_recovery,
};
