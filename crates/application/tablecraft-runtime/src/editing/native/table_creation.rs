mod declaration;

use crate::editing::SaveSchemaChange;
use crate::editing::{CreatedTableReopen, ReviewedTableCreation};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    document::Document,
    editing::{EditError, platform::ids::issue_id},
};
use bytes::Bytes;
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{FirstTableDefinition, NativeImage, NativeImageSource};
use tablecraft_builders::table::TableDraftSnapshot;
use tablecraft_core::limits::{Resource, ResourceBudget};

const REOPEN_COPY_CHUNK_BYTES: usize = 16 * 1024;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<CreatedTableReopen>,
    pub(in crate::editing) original: Bytes,
    pub(in crate::editing) definition: FirstTableDefinition,
    pub(in crate::editing) valid: Arc<AtomicBool>,
    pub(in crate::editing) profile: TableCreationProfile,
}

/// Captured from the current document's source qualification, never caller input.
#[derive(Clone, Copy)]
pub(crate) enum TableCreationProfile {
    First,
    Added,
}

impl TableCreationProfile {
    pub(in crate::editing) fn schema_change(self, column_count: usize) -> SaveSchemaChange {
        match self {
            Self::First => SaveSchemaChange::FirstTableCreated { column_count },
            Self::Added => SaveSchemaChange::TableAdded { column_count },
        }
    }
}

impl Document {
    pub(crate) fn review_table_creation(
        &mut self,
        document_revision: u64,
        snapshot: &TableDraftSnapshot,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let _span = tracing::debug_span!("application.review_table_creation").entered();
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }
        let profile = self.table_creation.ok_or(EditError::Unavailable)?;
        let mut work = budget();
        check(cancelled)?;
        let definition = declaration::capture(snapshot, &mut work, cancelled)?;
        // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this
        // shares immutable bytes instead of copying the database image.
        let source = self.edits.image_source.clone();
        let image = match profile {
            TableCreationProfile::First => {
                NativeImage::capture_shared(source, SCOPE, &mut work, cancelled)
            }
            TableCreationProfile::Added => {
                NativeImage::capture_shared_for_table_append(source, SCOPE, &mut work, cancelled)
            }
        }?;
        let (expected, columns, definition) = match profile {
            TableCreationProfile::First => {
                let plan = image.plan_first_table(definition, &mut work, cancelled)?;
                let expected = copy_expected(plan.after_bytes(), &mut work, cancelled)?;
                (expected, plan.column_count(), plan.into_definition())
            }
            TableCreationProfile::Added => {
                let plan = image.plan_add_table(definition, &mut work, cancelled)?;
                let expected = copy_expected(plan.after_bytes(), &mut work, cancelled)?;
                (expected, plan.column_count(), plan.into_definition())
            }
        };
        work.charge_allocation(self.edits.path.as_os_str().len() as u64, 1)?;
        work.charge_allocation(snapshot.table_name().len() as u64, 1)?;
        let mut table = String::new();
        table.try_reserve_exact(snapshot.table_name().len())?;
        table.push_str(snapshot.table_name());
        work.charge_allocation(1, size_of::<CreatedTableReopen>() as u64)?;
        work.charge_allocation(1, size_of::<State>() as u64)?;
        check(cancelled)?;
        let expected = NativeImageSource::from_vec(expected, &mut work, cancelled)?.shared_bytes();
        let reopen = Arc::new(CreatedTableReopen {
            // NO_CLONE: The capability or reopen witness outlives this borrowed Document call
            // and owns its source path; the live Document keeps its own path.
            path: self.edits.path.clone(),
            expected,
            table,
            columns,
            document_revision,
            draft_revision: snapshot.revision(),
            schema_change: profile.schema_change(columns),
        });
        let capability = ReviewedTableCreation {
            state: Box::new(State {
                id: issue_id()?,
                reopen,
                // NO_CLONE: The reader and retained native witness need independent lifetimes;
                // Bytes shares the immutable original image rather than copying it.
                original: self.edits.original.clone(),
                definition,
                profile,
                valid: Arc::clone(&self.edits.valid),
            }),
        };
        Ok(Response::TableCreationReviewed { capability })
    }
}

// Expected AFTER is content evidence for reopening, never a save payload. The
// consuming save reconstructs the selected operation under its exclusive lease.
fn copy_expected(
    after: &[u8],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    work.charge_allocation(after.len() as u64, 1)?;
    let mut expected = Vec::new();
    expected.try_reserve_exact(after.len())?;
    for chunk in after.chunks(REOPEN_COPY_CHUNK_BYTES) {
        check(cancelled)?;
        work.charge(Resource::Steps, chunk.len() as u64)?;
        expected.extend_from_slice(chunk);
    }
    Ok(expected)
}

#[cfg(test)]
mod tests;
