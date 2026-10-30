pub(in crate::execution) mod assignment;
mod error;
mod plan;
pub(in crate::execution) mod source;

pub use error::UpdateError;
pub use plan::{UpdatePlan, plan_batch_update, plan_update};
