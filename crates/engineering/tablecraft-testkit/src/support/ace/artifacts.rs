//! Finish explicit fixture exports without claiming database-publication durability.
use std::{fs, io, path::Path};

/// The actual directory operation completed after callers flushed each artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportNamespace {
    /// Unix directory synchronization completed successfully.
    Synchronized,
    /// No directory durability barrier was requested on this platform.
    NotSynchronized,
}

/// Finish an explicitly created fixture directory after every artifact file has
/// been flushed and closed by its exporter. Unix also syncs the directory.
/// Windows and other platforms validate its kind but provide no namespace
/// durability guarantee. This is not the required database publisher or journal.
pub fn finish_export_directory(directory: &Path) -> io::Result<ExportNamespace> {
    if !fs::metadata(directory)?.is_dir() {
        return Err(io::ErrorKind::NotADirectory.into());
    }
    #[cfg(unix)]
    {
        fs::File::open(directory)?.sync_all()?;
        Ok(ExportNamespace::Synchronized)
    }
    #[cfg(not(unix))]
    {
        Ok(ExportNamespace::NotSynchronized)
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportNamespace, finish_export_directory};
    use std::{env, io::ErrorKind};

    #[test]
    fn real_directory_reports_actual_platform_operation_and_files_refuse() {
        let completed = finish_export_directory(&env::temp_dir()).unwrap();
        #[cfg(unix)]
        assert_eq!(completed, ExportNamespace::Synchronized);
        #[cfg(not(unix))]
        assert_eq!(completed, ExportNamespace::NotSynchronized);

        assert_eq!(
            finish_export_directory(&env::current_exe().unwrap())
                .unwrap_err()
                .kind(),
            ErrorKind::NotADirectory,
        );
    }
}
