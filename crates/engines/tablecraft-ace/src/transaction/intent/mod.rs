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
pub use eof_insert::{
    NativeEofInsertIntent, QualifiedNativeEofInsertRecovery, RecoverableNativeEofInsertPlan,
    RecoverableNativeInsertOperation, qualify_native_eof_insert_recovery,
};
