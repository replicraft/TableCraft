//! Stable, bounded file capture and read-session ownership.
use super::{
    Error, Response, SavedQuery, Table,
    controls::{SCOPE, budget, check},
    display::Presentation,
    source::Source,
};
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use crate::CreatedTableReopen;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
use crate::editing::platform::state::DocumentEdits;
#[cfg(all(
    feature = "offline-edit",
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
use crate::editing::platform::table_creation::TableCreationProfile;
#[cfg(feature = "ace16-create")]
use crate::{CreationOutcome, CreationReport};
#[cfg(feature = "offline-edit")]
use crate::{InsertRetryWitness, editing::journal_path};
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
use std::sync::atomic::AtomicBool;
#[cfg(feature = "offline-edit")]
use std::{fs::symlink_metadata, io::ErrorKind};
use std::{
    path::{Path, absolute},
    sync::Arc,
};
use tablecraft_access::NativeBrowser;
#[cfg(feature = "offline-edit")]
use tablecraft_ace::InspectError;
use tablecraft_ace::NativeTableStorage;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::create_blank_ace16_database;
#[cfg(all(
    feature = "offline-edit",
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
use tablecraft_ace::{add_table_source_capabilities, first_table_source_available};
use tablecraft_core::limits::Resource;
use tablecraft_io::SharedFile;
use tablecraft_model::ObjectKind;
use zeroize::Zeroizing;

pub(crate) const SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;
const COPY_CHUNK: usize = 64 * 1024;

pub(super) struct Document {
    pub(crate) filter_source: Arc<()>,
    pub(super) source: Source,
    pub(super) revision: u64,
    #[cfg(all(
        feature = "offline-edit",
        any(target_os = "macos", target_os = "linux", windows)
    ))]
    pub(crate) edits: DocumentEdits,
    #[cfg(all(
        feature = "offline-edit",
        feature = "ace16-create",
        any(target_os = "macos", target_os = "linux", windows)
    ))]
    pub(crate) table_creation: Option<TableCreationProfile>,
}

impl Document {
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    pub(super) fn open_database_options_saved(
        witness: &crate::DatabaseOptionsReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            Error::CreatedSourceChanged | Error::SourceChanged => {
                Error::DatabaseOptionsSourceChanged
            }
            error => error,
        })
    }

    #[cfg(feature = "offline-edit")]
    pub(super) fn open_query_expression_saved(
        witness: &crate::QueryExpressionReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            #[cfg(feature = "ace16-create")]
            Error::CreatedSourceChanged => Error::QueryExpressionSourceChanged,
            Error::SourceChanged => Error::QueryExpressionSourceChanged,
            error => error,
        })
    }

    #[cfg(feature = "offline-edit")]
    pub(super) fn open_table_renamed(
        witness: &crate::TableRenameReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            #[cfg(feature = "ace16-create")]
            Error::CreatedSourceChanged => Error::RenamedTableSourceChanged,
            Error::SourceChanged => Error::RenamedTableSourceChanged,
            error => error,
        })
    }

    #[cfg(feature = "offline-edit")]
    pub(super) fn open_insert_original(
        witness: &InsertRetryWitness,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            #[cfg(feature = "ace16-create")]
            Error::CreatedSourceChanged => Error::InsertOriginalSourceChanged,
            Error::SourceChanged => Error::InsertOriginalSourceChanged,
            error => error,
        })
    }

    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    pub(super) fn open_field_dropped(
        witness: &crate::FieldDroppedReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            Error::CreatedSourceChanged | Error::SourceChanged => Error::FieldDroppedSourceChanged,
            error => error,
        })
    }

    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    pub(super) fn open_field_renamed(
        witness: &crate::FieldRenamedReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            Error::CreatedSourceChanged | Error::SourceChanged => Error::FieldRenamedSourceChanged,
            error => error,
        })
    }

    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    pub(super) fn open_column_added(
        witness: &crate::AddedColumnReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            Error::CreatedSourceChanged | Error::SourceChanged => Error::AddedColumnSourceChanged,
            error => error,
        })
    }

    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    pub(super) fn open_table_created(
        witness: &CreatedTableReopen,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(
            witness.source_path(),
            &mut String::new(),
            revision,
            cancelled,
            Some(witness.expected_bytes()),
        )
        .map_err(|error| match error {
            Error::CreatedSourceChanged => Error::SavedTableSourceChanged,
            error => error,
        })
    }

    pub(super) fn open(
        path: &Path,
        password: &mut String,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Self::open_with_expected(path, password, revision, cancelled, None)
    }

    #[cfg(feature = "ace16-create")]
    pub(super) fn open_created(
        report: &CreationReport,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        if !matches!(report.outcome(), CreationOutcome::Created(_)) {
            return Err(Error::NoCreatedReceipt);
        }
        let mut work = budget();
        let expected = create_blank_ace16_database(&mut work, cancelled)?;
        Self::open_with_expected(
            report.destination(),
            &mut String::new(),
            revision,
            cancelled,
            Some(expected.as_bytes()),
        )
    }

    fn open_with_expected(
        path: &Path,
        password: &mut String,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
        expected: Option<&[u8]>,
    ) -> Result<(Self, Response), Error> {
        // Take credentials immediately, including on path or snapshot failures.
        let password = Zeroizing::new(std::mem::take(password));
        let mut work = budget();
        check(cancelled)?;
        let source_path = absolute(path).map_err(tablecraft_ace::InspectError::from)?;
        #[cfg(feature = "offline-edit")]
        match symlink_metadata(journal_path(&source_path)) {
            Ok(_) => return Err(Error::RetainedJournal),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(InspectError::from(error).into()),
        }
        let mut file = SharedFile::open(path)?;
        let length = file.length()?;
        if length > SNAPSHOT_BYTES {
            return Err(Error::SnapshotLimit);
        }
        work.charge(Resource::Bytes, length)?;
        let size = usize::try_from(length).map_err(|_| Error::SnapshotLimit)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size)?;
        bytes.resize(size, 0);
        for chunk in bytes.chunks_mut(COPY_CHUNK) {
            tablecraft_io::read_exact(&mut file, chunk, &mut work, cancelled)?;
        }
        if file.length()? != length {
            return Err(Error::SourceChanged);
        }
        file.close()?;
        check(cancelled)?;
        if let Some(expected) = expected {
            let mut matches = expected.len() == bytes.len();
            for (original, captured) in expected.chunks(COPY_CHUNK).zip(bytes.chunks(COPY_CHUNK)) {
                check(cancelled)?;
                work.charge(Resource::Steps, original.len() as u64)?;
                matches &= original == captured;
            }
            check(cancelled)?;
            if !matches {
                #[cfg(feature = "ace16-create")]
                return Err(Error::CreatedSourceChanged);
                #[cfg(not(feature = "ace16-create"))]
                return Err(Error::SourceChanged);
            }
        }
        let image_source =
            tablecraft_ace::NativeImageSource::from_vec(bytes, &mut work, cancelled)?;
        let bytes = image_source.shared_bytes();
        #[cfg(all(
            feature = "offline-edit",
            any(target_os = "macos", target_os = "linux", windows)
        ))]
        let edits = {
            work.charge_allocation(source_path.as_os_str().len() as u64, 1)?;
            work.charge_allocation(1, size_of::<AtomicBool>() as u64)?;
            // NO_CLONE: The capability or reopen witness outlives this borrowed Document call
            // and owns its source path; the live Document keeps its own path.
            DocumentEdits::new(source_path.clone(), bytes.clone(), image_source)
        };
        // NO_CLONE: The reader and retained native witness need independent lifetimes; Bytes
        // shares the immutable original image rather than copying it.
        #[cfg(all(
            feature = "offline-edit",
            feature = "ace16-create",
            any(target_os = "macos", target_os = "linux", windows)
        ))]
        let captured = bytes.clone();
        let (source, format) = Source::open(bytes, password.as_str(), &mut work, cancelled)?;
        #[cfg(all(
            feature = "offline-edit",
            feature = "ace16-create",
            any(target_os = "macos", target_os = "linux", windows)
        ))]
        let (table_creation, table_creation_available, table_design_available) =
            if !source.editable_native() {
                (None, false, false)
            } else if first_table_source_available(&captured, &mut work, cancelled)? {
                (Some(TableCreationProfile::First), true, true)
            } else {
                let capabilities =
                    add_table_source_capabilities(&captured, SCOPE, &mut work, cancelled)?;
                let keyed = capabilities.can_add_keyed();
                let design = keyed || capabilities.can_add_keyless();
                (design.then_some(TableCreationProfile::Added), keyed, design)
            };
        #[cfg(not(all(
            feature = "offline-edit",
            feature = "ace16-create",
            any(target_os = "macos", target_os = "linux", windows)
        )))]
        let (table_creation_available, table_design_available) = (false, false);
        let mut document = Self {
            filter_source: Arc::new(()),
            source,
            revision,
            #[cfg(all(
                feature = "offline-edit",
                any(target_os = "macos", target_os = "linux", windows)
            ))]
            edits,
            #[cfg(all(
                feature = "offline-edit",
                feature = "ace16-create",
                any(target_os = "macos", target_os = "linux", windows)
            ))]
            table_creation,
        };
        let mut browser = NativeBrowser::new(&mut document.source, SCOPE, work, cancelled);
        let inventory = browser.inventory()?;
        let mut output = Presentation::new(cancelled);
        let mut table_count = 0_usize;
        let mut query_count = 0_usize;
        for entry in inventory.entries() {
            output.work(1)?;
            match entry.object().kind() {
                ObjectKind::Table => table_count += 1,
                ObjectKind::Query => query_count += 1,
                _ => {}
            }
        }
        output.allocation(table_count, size_of::<Table>())?;
        output.allocation(query_count, size_of::<SavedQuery>())?;
        let mut tables = Vec::new();
        tables.try_reserve_exact(table_count)?;
        let mut queries = Vec::new();
        queries.try_reserve_exact(query_count)?;
        for entry in inventory.entries() {
            output.work(1)?;
            match entry.object().kind() {
                ObjectKind::Table => tables.push(Table {
                    local: matches!(entry.storage(), Some(NativeTableStorage::Local)),
                    name: output.text(entry.object().name().as_str())?,
                    system: entry.is_system(),
                    storage: output.text(&format!("{:?}", entry.storage()))?,
                }),
                ObjectKind::Query => queries.push(SavedQuery {
                    name: output.text(entry.object().name().as_str())?,
                    object_id: entry.object().id().as_u128(),
                    system: entry.is_system(),
                }),
                _ => {}
            }
        }
        check(cancelled)?;
        let title = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Ok((
            document,
            Response::Opened {
                document_revision: revision,
                source_path,
                table_creation_available,
                table_design_available,
                title,
                format,
                tables,
                queries,
            },
        ))
    }
}
