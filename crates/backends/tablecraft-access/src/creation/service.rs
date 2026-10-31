use super::CreationError;
use std::{fmt, path::Path};
use tablecraft_ace::{
    CreateColumn, CreatedDatabase, create_ace16_database, create_ace16_int32_database,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::publication::{ParentIdentity, PublicationReceipt, RequiredOutput};

/// Captured destination for one no-clobber, namespace-durable creation.
/// Capture creates no entry. Unsupported backends refuse without downgrading.
pub struct CreationDestination {
    output: RequiredOutput,
}

/// Native construction is complete; publication has not yet been attempted.
/// Hosts can allocate their entire completion report from these counts first.
pub struct PreparedCreation {
    output: RequiredOutput,
    database: CreatedDatabase,
}

/// Acknowledged fresh publication and captured-directory barrier.
/// This is not a normal ACE edit, journal plan or native Access save receipt.
pub struct CreationReceipt {
    publication: PublicationReceipt,
    rows: usize,
    columns: usize,
}

impl CreationDestination {
    /// Capture the final parent and preflight its barrier without creating files.
    /// The host maintains the namespace assumptions documented by RequiredOutput.
    pub fn capture(
        path: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreationError> {
        let _span = tracing::debug_span!("access.creation.capture_destination").entered();
        Ok(Self {
            output: RequiredOutput::capture(path, budget, cancelled)?,
        })
    }

    /// Construct one explicit ordinary Int32 schema and its complete native image.
    /// Invalid inputs create no temporary or final file. No template is accepted.
    pub fn prepare(
        self,
        table: &str,
        columns: &[&str],
        rows: &[&[i32]],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedCreation, CreationError> {
        let _span = tracing::debug_span!("access.creation.prepare").entered();
        let database = create_ace16_int32_database(table, columns, rows, budget, cancelled)?;
        Ok(PreparedCreation {
            output: self.output,
            database,
        })
    }

    /// Construct one explicit ordinary fixed-scalar schema from exact carriers.
    /// Column declarations remain authoritative for empty/all-NULL input. The
    /// native creator rejects unsupported families, carrier mismatches and
    /// Boolean NULL before any output is staged. No normal ACE edit or imported
    /// image authority is granted; publication uses the same required protocol.
    pub fn prepare_typed(
        self,
        table: &str,
        columns: &[CreateColumn<'_>],
        rows: &[&[Value]],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedCreation, CreationError> {
        let _span = tracing::debug_span!("access.creation.prepare_typed").entered();
        let database = create_ace16_database(table, columns, rows, budget, cancelled)?;

        Ok(PreparedCreation {
            output: self.output,
            database,
        })
    }
}

impl PreparedCreation {
    /// Number of complete logical rows in the unpublished image.
    pub fn row_count(&self) -> usize {
        self.database.row_count()
    }

    /// Number of ordinary fixed-scalar columns in the unpublished image.
    pub fn column_count(&self) -> usize {
        self.database.column_count()
    }

    /// Publish once under the captured fresh name, then finish its barriers.
    /// Every error retains its exact IO publication fact; no retry or downgrade
    /// occurs and no final entry is automatically removed after publication.
    pub fn publish(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CreationReceipt, CreationError> {
        let _span = tracing::debug_span!("access.creation.publish").entered();
        let rows = self.database.row_count();
        let columns = self.database.column_count();
        let publication = self
            .output
            .publish(self.database.as_bytes(), budget, cancelled)?;
        Ok(CreationReceipt {
            publication,
            rows,
            columns,
        })
    }
}

impl CreationReceipt {
    /// Number of complete rows in the acknowledged published file.
    pub const fn row_count(&self) -> usize {
        self.rows
    }

    /// Number of ordinary fixed-scalar columns in the acknowledged published file.
    pub const fn column_count(&self) -> usize {
        self.columns
    }

    /// The captured directory acknowledged its terminal namespace barrier.
    pub const fn namespace_synced(&self) -> bool {
        self.publication.namespace_synced()
    }

    /// Captured parent identity, independent of later textual path retargeting.
    pub fn parent_identity(&self) -> &ParentIdentity {
        self.publication.parent_identity()
    }
}

impl fmt::Debug for CreationReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreationReceipt")
            .field("rows", &self.rows)
            .field("columns", &self.columns)
            .field("namespace_synced", &self.namespace_synced())
            .finish_non_exhaustive()
    }
}
