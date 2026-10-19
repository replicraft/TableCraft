use thiserror::Error;

/// Adapter edit failures retain sanitized typed causes, never names or values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativeEditError {
    /// Native capture/read failure.
    #[error(transparent)]
    Inspect(#[from] tablecraft_ace::InspectError),
    /// Snapshot identity/schema failure.
    #[error(transparent)]
    Snapshot(#[from] tablecraft_db::SnapshotError),
    /// Native constraint/preservation/profile failure.
    #[error(transparent)]
    Edit(#[from] tablecraft_ace::EditError),
    /// Requested carrier or column has no implemented assignment profile.
    #[error("native assignment carrier is unsupported")]
    Unsupported,
}
