use super::{installed::MAX_PAYLOAD_BYTES, report::Report};
use crate::UpdateError;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

const MAX_MEMBERS: usize = 4;
const MAX_NAME_BYTES: usize = 128;
const BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn extract(
    source: &Path,
    output: &Path,
    report: &Report,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), UpdateError> {
    let file = File::open(source).map_err(UpdateError::Io)?;
    let metadata = file.metadata().map_err(UpdateError::Io)?;
    if !metadata.is_file() || metadata.len() > MAX_PAYLOAD_BYTES {
        return Err(UpdateError::Archive);
    }
    let mut cabinet = cab::Cabinet::new(file).map_err(|_| UpdateError::Archive)?;
    let mut members = BTreeMap::new();
    let mut total = 0_u64;
    let mut folders = 0;
    for folder in cabinet.folder_entries() {
        folders += 1;
        if folders > MAX_MEMBERS {
            return Err(UpdateError::Archive);
        }
        for entry in folder.file_entries() {
            if cancelled() {
                return Err(UpdateError::Cancelled);
            }
            if members.len() >= MAX_MEMBERS || entry.name().len() > MAX_NAME_BYTES {
                return Err(UpdateError::Archive);
            }
            let definition = report
                .files
                .iter()
                .find(|file| file.id == entry.name())
                .ok_or(UpdateError::Archive)?;
            let size = u64::from(entry.uncompressed_size());
            if size == 0 || definition.size.parse::<u64>().ok() != Some(size) {
                return Err(UpdateError::Archive);
            }
            total = total.checked_add(size).ok_or(UpdateError::AssetTooLarge)?;
            if total > MAX_PAYLOAD_BYTES
                || members
                    .insert(entry.name().to_string(), definition)
                    .is_some()
            {
                return Err(UpdateError::Archive);
            }
        }
    }
    if members.len() != MAX_MEMBERS {
        return Err(UpdateError::Archive);
    }
    fs::create_dir(output).map_err(UpdateError::Io)?;
    let mut written = 0_u64;
    let mut buffer = [0; BUFFER_BYTES];
    for (id, definition) in members {
        let name = definition
            .name
            .split('|')
            .next_back()
            .ok_or(UpdateError::Archive)?;
        if ![
            "tablecraft-desktop.exe",
            "LICENSE",
            "ThirdPartyLicenses.txt",
            "RELEASE.json",
        ]
        .contains(&name)
        {
            return Err(UpdateError::Archive);
        }
        let size = definition
            .size
            .parse::<u64>()
            .map_err(|_| UpdateError::Archive)?;
        let mut reader = cabinet.read_file(&id).map_err(|_| UpdateError::Archive)?;
        let mut destination = File::create_new(output.join(name)).map_err(UpdateError::Io)?;
        let mut remaining = size;
        while remaining != 0 {
            if cancelled() {
                return Err(UpdateError::Cancelled);
            }
            let length = usize::try_from(remaining.min(BUFFER_BYTES as u64))
                .map_err(|_| UpdateError::Archive)?;
            let count = reader
                .read(&mut buffer[..length])
                .map_err(|_| UpdateError::Archive)?;
            if count == 0 {
                return Err(UpdateError::Archive);
            }
            destination
                .write_all(&buffer[..count])
                .map_err(UpdateError::Io)?;
            remaining -= count as u64;
            written += count as u64;
            if written > total {
                return Err(UpdateError::AssetTooLarge);
            }
        }
        if reader.read(&mut [0]).map_err(|_| UpdateError::Archive)? != 0 {
            return Err(UpdateError::Archive);
        }
        destination.sync_all().map_err(UpdateError::Io)?;
    }
    if written != total {
        return Err(UpdateError::Archive);
    }
    Ok(())
}
