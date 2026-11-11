pub(in crate::execution) mod assignment;
mod error;
mod plan;

pub use error::UpdateError;
pub use plan::{
    UpdatePlan, plan_batch_update, plan_batch_update_with_budgets, plan_batch_update_with_options,
    plan_update,
};

mod joined;
pub use joined::{JoinedMutationOptions, plan_joined_batch_update_with_options};
