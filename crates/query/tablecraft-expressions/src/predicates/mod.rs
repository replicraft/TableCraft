//! Shared eager range/membership rules; hosts supply bounded comparison policy.
mod membership;
mod range;
pub use membership::Membership;
pub use range::between;
