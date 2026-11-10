//! Current column additions use explicitly selected closed request languages.
mod candidate;
mod codec;
mod planning;
mod record;
mod recovery;

pub use planning::RecoverableCurrentFieldAdditionPlan;
pub use record::{CurrentFieldAdditionIntentError, NativeCurrentFieldAdditionIntent};
pub use recovery::{
    QualifiedCurrentFieldAdditionRecovery, qualify_native_current_field_addition_recovery,
};

#[cfg(test)]
mod tests;
