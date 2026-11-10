//! Current column additions use explicitly selected closed request languages.
#[cfg(feature = "ace16-create")]
mod candidate;
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
mod record;
mod recovery;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableCurrentFieldAdditionPlan;
pub use record::{CurrentFieldAdditionIntentError, NativeCurrentFieldAdditionIntent};
pub use recovery::QualifiedRetainedFieldAdditionRecovery;

#[cfg(test)]
mod tests;

#[cfg(feature = "ace16-create")]
pub use recovery::{
    QualifiedCurrentFieldAdditionRecovery, qualify_native_current_field_addition_recovery,
};
