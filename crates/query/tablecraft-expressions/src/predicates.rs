//! Shared eager range/membership rules; hosts supply bounded comparison policy.
mod like;
mod membership;
mod range;
pub use like::{
    LikeCase, LikeMatch, LikeMode, PreparedLikeSearch, find_like_utf16, like, like_utf16,
    next_like_offset, previous_like_offset,
};
pub use membership::Membership;
pub use range::between;
