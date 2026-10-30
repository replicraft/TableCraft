//! Format-independent bounded synchronous I/O and owned OS file locks.
//!
//! Callers charge requested bytes before I/O; helpers charge callback work.
//! Individual blocking callbacks remain responsible for their own timeouts.

mod error;
mod lock;
pub use error::IoError;
pub use lock::{ExclusiveFile, SharedFile};
mod progress;
mod seek;
pub use seek::{seek_absolute, seek_to_end};
mod read;
pub use read::{read_exact, read_exact_at};
mod write;
pub use write::write_exact_at;
/// Optional fixed and resizable image journals over explicitly owned storage.
#[cfg(feature = "journal")]
pub mod journal;
