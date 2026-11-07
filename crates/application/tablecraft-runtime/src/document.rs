//! Stable, bounded file capture and read-session ownership.
use super::{
    Error, Response, SavedQuery, Table,
    controls::{SCOPE, budget, check},
    display::Presentation,
    source::Source,
};
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use crate::FirstTableReopen;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
use crate::editing::DocumentEdits;
#[cfg(feature = "offline-edit")]
use crate::editing::journal_path;
#[cfg(feature = "ace16-create")]
use crate::{CreationOutcome, CreationReport};
use bytes::Bytes;
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
use tablecraft_core::limits::Resource;
use tablecraft_io::SharedFile;
use tablecraft_model::ObjectKind;
use zeroize::Zeroizing;

const SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;
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
}

impl Document {
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    pub(super) fn open_first_table_saved(
        witness: &FirstTableReopen,
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
        if expected.is_some_and(|expected| expected != bytes.as_slice()) {
            #[cfg(feature = "ace16-create")]
            return Err(Error::CreatedSourceChanged);
            #[cfg(not(feature = "ace16-create"))]
            return Err(Error::SourceChanged);
        }
        let bytes = Bytes::from(bytes);
        #[cfg(all(
            feature = "offline-edit",
            any(target_os = "macos", target_os = "linux", windows)
        ))]
        let edits = {
            work.charge_allocation(source_path.as_os_str().len() as u64, 1)?;
            work.charge_allocation(1, size_of::<AtomicBool>() as u64)?;
            DocumentEdits::new(source_path.clone(), bytes.clone())
        };
        let (source, format) = Source::open(bytes, password.as_str(), &mut work, cancelled)?;
        let mut document = Self {
            filter_source: Arc::new(()),
            source,
            revision,
            #[cfg(all(
                feature = "offline-edit",
                any(target_os = "macos", target_os = "linux", windows)
            ))]
            edits,
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
                title,
                format,
                tables,
                queries,
            },
        ))
    }
}
