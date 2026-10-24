//! Shared eager range/membership rules; hosts supply bounded comparison policy.
mod like;
mod membership;
pub use like::{LikeMode, LikeString, like, like_utf16};
mod range;
pub use membership::Membership;
pub use range::between;
