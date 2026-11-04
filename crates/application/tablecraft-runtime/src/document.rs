//! Stable, bounded file capture and read-session ownership.
use super::{
    Error, Response, Table,
    controls::{SCOPE, budget, check},
    source::Source,
};
#[cfg(feature = "offline-edit")]
use crate::editing::{DocumentEdits, journal_path};
use bytes::Bytes;
use std::path::Path;
#[cfg(feature = "offline-edit")]
use std::{fs::symlink_metadata, io::ErrorKind, path::absolute, sync::atomic::AtomicBool};
use tablecraft_access::NativeBrowser;
#[cfg(feature = "offline-edit")]
use tablecraft_ace::InspectError;
use tablecraft_core::limits::Resource;
use tablecraft_io::SharedFile;
use zeroize::Zeroizing;

const SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;
const COPY_CHUNK: usize = 64 * 1024;

pub(super) struct Document {
    pub(super) source: Source,
    pub(super) revision: u64,
    #[cfg(feature = "offline-edit")]
    pub(crate) edits: DocumentEdits,
}

impl Document {
    pub(super) fn open(
        path: &Path,
        password: &mut String,
        revision: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        // Take credentials immediately, including on path or snapshot failures.
        let password = Zeroizing::new(std::mem::take(password));
        let mut work = budget();
        check(cancelled)?;
        #[cfg(feature = "offline-edit")]
        let retained_path = absolute(path).map_err(InspectError::from)?;
        #[cfg(feature = "offline-edit")]
        match symlink_metadata(journal_path(&retained_path)) {
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
        let bytes = Bytes::from(bytes);
        #[cfg(feature = "offline-edit")]
        let edits = {
            work.charge_allocation(retained_path.as_os_str().len() as u64, 1)?;
            work.charge_allocation(1, size_of::<AtomicBool>() as u64)?;
            DocumentEdits::new(retained_path, bytes.clone())
        };
        let (source, format) = Source::open(bytes, password.as_str(), &mut work, cancelled)?;
        let mut document = Self {
            source,
            revision,
            #[cfg(feature = "offline-edit")]
            edits,
        };
        let mut browser = NativeBrowser::new(&mut document.source, SCOPE, work, cancelled);
        let inventory = browser.tables()?;
        let tables = inventory
            .entries()
            .map(|entry| Table {
                name: entry.object().name().as_str().to_owned(),
                system: entry.is_system(),
                storage: format!("{:?}", entry.storage()),
            })
            .collect();
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
                title,
                format,
                tables,
            },
        ))
    }
}
