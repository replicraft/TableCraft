//! Exact scalar carriers. Equality compares representation, not Access SQL values.
//!
//! Field constraints, collations, coercions, codecs and arithmetic promotion are
//! separate responsibilities. Text is decoded Unicode; native source bytes and
//! encoding metadata must be preserved by the backend, not reconstructed here.

pub mod currency;
pub mod datetime;
pub mod decimal;
pub mod expression;
pub mod floating_point;
pub mod guid;
pub mod stored;
pub mod validation;

mod fixed;

pub use currency::Currency;
pub use datetime::{ExtendedDateTime, LegacyDateTime};
pub use decimal::Decimal;
pub use expression::ExpressionValue;
pub use floating_point::{Double, Single};
pub use guid::Guid;
pub use stored::Value;
pub use validation::{ExpressionState, ValueError};
