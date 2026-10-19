//! Shared eager range/membership rules; hosts supply bounded comparison policy.
mod like;
mod membership;
pub use like::{LikeMode, like};
mod range;
pub use membership::Membership;
pub use range::between;
