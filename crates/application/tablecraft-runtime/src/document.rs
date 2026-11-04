//! Stable, bounded file capture and read-session ownership.
use super::{
    Error, Response, Table,
    controls::{SCOPE, budget, check},
    source::Source,
};
use std::path::Path;
use tablecraft_access::NativeBrowser;
use tablecraft_core::limits::Resource;
use tablecraft_io::SharedFile;
use zeroize::Zeroizing;

const SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;
const COPY_CHUNK: usize = 64 * 1024;

pub(super) struct Document {
    pub(super) source: Source,
}

impl Document {
    pub(super) fn open(
        path: &Path,
        password: &mut String,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        // Take credentials immediately, including on path or snapshot failures.
        let password = Zeroizing::new(std::mem::take(password));
        let mut work = budget();
        check(cancelled)?;
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
        let (source, format) = Source::open(bytes, password.as_str(), &mut work, cancelled)?;
        let mut document = Self { source };
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
                title,
                format,
                tables,
            },
        ))
    }
}
