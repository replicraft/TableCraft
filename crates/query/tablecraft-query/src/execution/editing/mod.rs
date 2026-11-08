pub(in crate::execution) mod assignment;
mod error;
mod plan;
pub(in crate::execution) mod source;

pub use error::UpdateError;
pub use plan::{
    UpdateComparisonOptions, UpdatePlan, plan_batch_update, plan_batch_update_with_budgets,
    plan_batch_update_with_comparison, plan_update,
};
