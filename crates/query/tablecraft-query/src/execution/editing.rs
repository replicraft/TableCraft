pub(in crate::execution) mod assignment;
mod error;
mod plan;

pub use error::UpdateError;
pub use plan::{
    UpdatePlan, plan_batch_update, plan_batch_update_with_budgets, plan_batch_update_with_options,
    plan_update,
};

mod joined;
pub use joined::{
    JoinedMutationOptions, PreparedJoinedUpdate, plan_joined_batch_update_with_options,
    prepare_joined_batch_update_with_options,
};

mod read_source;
pub use read_source::{
    PreparedReadSourceUpdate, ReadSourceUpdateOptions,
    prepare_read_source_batch_update_with_options,
};
