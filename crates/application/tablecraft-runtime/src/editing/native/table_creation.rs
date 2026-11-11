mod declaration;

use crate::editing::SaveSchemaChange;
use crate::editing::{CreatedTableReopen, ReviewedTableCreation, table_creation::CreationOrigin};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    document::Document,
    editing::{EditError, platform::ids::issue_id},
};
use bytes::Bytes;
use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_ace::{FirstTableDefinition, NativeImage, NativeImageSource, PopulatedAddTablePlan};
use tablecraft_builders::table::TableDraftSnapshot;
use tablecraft_core::limits::{Resource, ResourceBudget};

const REOPEN_COPY_CHUNK_BYTES: usize = 16 * 1024;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<CreatedTableReopen>,
    pub(in crate::editing) request: Request,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(in crate::editing) enum Request {
    Empty {
        original: Bytes,
        definition: FirstTableDefinition,
        profile: TableCreationProfile,
    },
    Populated(PopulatedAddTablePlan),
}

impl Request {
    fn table(&self) -> &str {
        match self {
            Self::Empty { definition, .. } => definition.table(),
            Self::Populated(plan) => plan.table(),
        }
    }

    fn column_count(&self) -> usize {
        match self {
            Self::Empty { definition, .. } => definition.column_count(),
            Self::Populated(plan) => plan.column_count(),
        }
    }

    fn inserted_rows(&self) -> Option<usize> {
        match self {
            Self::Empty { .. } => None,
            Self::Populated(plan) => Some(plan.row_count()),
        }
    }

    fn schema_change(&self) -> SaveSchemaChange {
        match self {
            Self::Empty { profile, .. } => profile.schema_change(self.column_count()),
            Self::Populated(_) => SaveSchemaChange::TableAdded {
                column_count: self.column_count(),
            },
        }
    }
}

/// Captured from the current document's source qualification, never caller input.
#[derive(Clone, Copy)]
pub(crate) enum TableCreationProfile {
    First,
    Added,
    AddedNative,
}

impl TableCreationProfile {
    pub(in crate::editing) fn schema_change(self, column_count: usize) -> SaveSchemaChange {
        match self {
            Self::First => SaveSchemaChange::FirstTableCreated { column_count },
            Self::Added | Self::AddedNative => SaveSchemaChange::TableAdded { column_count },
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
        let definition = declaration::capture(snapshot, profile, &mut work, cancelled)?;
        self.review_native_definition(
            document_revision,
            definition,
            CreationOrigin::Design {
                draft_revision: snapshot.revision(),
            },
            &mut work,
            cancelled,
        )
    }

    /// Lowered declarations share the native planner, save capability and budget
    /// with actual table-design reviews; no synthetic design snapshot is needed.
    pub(in crate::editing) fn review_native_definition(
        &mut self,
        document_revision: u64,
        definition: FirstTableDefinition,
        origin: CreationOrigin,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }
        let profile = self.table_creation.ok_or(EditError::Unavailable)?;
        check(cancelled)?;
        // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this
        // shares immutable bytes instead of copying the database image.
        let source = self.edits.image_source.clone();
        let image = match (profile, definition.is_native()) {
            (TableCreationProfile::First, false) => {
                NativeImage::capture_shared(source, SCOPE, work, cancelled)
            }
            _ => NativeImage::capture_shared_retained(source, SCOPE, work, cancelled),
        }?;
        let (expected, definition) = match profile {
            TableCreationProfile::First => {
                let plan = if definition.is_native() {
                    image.plan_first_table_native(definition, work, cancelled)?
                } else {
                    image.plan_first_table(definition, work, cancelled)?
                };
                let expected = copy_expected(plan.after_bytes(), work, cancelled)?;
                (expected, plan.into_definition())
            }
            TableCreationProfile::Added | TableCreationProfile::AddedNative => {
                let plan = image.plan_authored_add_table(definition, work, cancelled)?;
                let expected = copy_expected(plan.after_bytes(), work, cancelled)?;
                (expected, plan.into_definition())
            }
        };
        let expected = NativeImageSource::from_vec(expected, work, cancelled)?.shared_bytes();
        // NO_CLONE: The consuming save retains its BEFORE independently from the
        // live Document; Bytes shares the complete immutable allocation.
        let original = self.edits.original.clone();
        self.issue_table_creation(
            document_revision,
            origin,
            Request::Empty {
                original,
                definition,
                profile,
            },
            expected,
            work,
            cancelled,
        )
    }

    pub(in crate::editing) fn issue_table_creation(
        &self,
        document_revision: u64,
        origin: CreationOrigin,
        request: Request,
        expected: Bytes,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        work.charge_allocation(self.edits.path.as_os_str().len() as u64, 1)?;
        work.charge_allocation(request.table().len() as u64, 1)?;
        let mut table = String::new();
        table.try_reserve_exact(request.table().len())?;
        table.push_str(request.table());
        work.charge_allocation(1, size_of::<CreatedTableReopen>() as u64)?;
        work.charge_allocation(1, size_of::<State>() as u64)?;
        check(cancelled)?;
        let reopen = Arc::new(CreatedTableReopen {
            // NO_CLONE: The read-only reopen witness outlives this Document call
            // and owns its path independently from the live reader.
            path: self.edits.path.clone(),
            expected,
            table,
            columns: request.column_count(),
            inserted_rows: request.inserted_rows(),
            document_revision,
            origin,
            schema_change: request.schema_change(),
        });
        Ok(Response::TableCreationReviewed {
            capability: ReviewedTableCreation {
                state: Box::new(State {
                    id: issue_id()?,
                    reopen,
                    request,
                    valid: Arc::clone(&self.edits.valid),
                }),
            },
        })
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
