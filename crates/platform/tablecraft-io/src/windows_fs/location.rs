//! Capture Windows paths once under the same required-publication qualification.
use super::{
    FileIdentity, MAX_COMPONENT_UNITS, identity, invalid, open_parent, qualify_local_ntfs,
};
use crate::{IoError, progress::check, windows_names as names};
use std::{
    fs::File,
    io::ErrorKind,
    mem::size_of,
    os::windows::ffi::OsStrExt,
    path::{Component, Path, Prefix},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_PATH_UNITS: usize = 32_767;
const PATH_SCAN_PASSES: u64 = 12;

pub(crate) struct Location {
    pub(crate) parent: File,
    pub(crate) name: Vec<u16>,
    pub(crate) identity: FileIdentity,
}

impl Location {
    pub(crate) fn capture(
        output: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IoError> {
        check(budget, cancelled)?;
        let input_length = output
            .as_os_str()
            .encode_wide()
            .take(MAX_PATH_UNITS + 1)
            .count();
        budget.charge(Resource::Steps, input_length as u64)?;
        if input_length > MAX_PATH_UNITS {
            return Err(invalid());
        }
        if matches!(output.as_os_str().encode_wide().last(), Some(47 | 92)) {
            return Err(invalid());
        }
        // Fully relative names resolve once. Drive-relative and root-relative
        // spellings depend on hidden per-drive state and remain refused.
        if !output.is_absolute()
            && output
                .components()
                .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
        {
            return Err(invalid());
        }
        // Validate the raw leaf before Win32 absolute-path normalization can
        // alter trailing dots/spaces or interpret a DOS device spelling.
        let mut raw_leaf = [0; MAX_COMPONENT_UNITS];
        let mut raw_leaf_length = 0;
        for unit in output.as_os_str().encode_wide() {
            if unit == b'\\' as u16 || unit == b'/' as u16 {
                raw_leaf_length = 0;
                continue;
            }
            if raw_leaf_length == raw_leaf.len() {
                return Err(invalid());
            }
            raw_leaf[raw_leaf_length] = unit;
            raw_leaf_length += 1;
        }
        if !names::valid_component(&raw_leaf[..raw_leaf_length]) {
            return Err(invalid());
        }
        // Windows resolution can include the process current directory.
        // Admit its platform maximum before invoking the allocating OS helper.
        budget.charge(
            Resource::Bytes,
            ((MAX_PATH_UNITS + 1) * size_of::<u16>()) as u64,
        )?;
        budget.charge(Resource::Steps, MAX_PATH_UNITS as u64)?;
        let resolved = std::path::absolute(output)?;
        let output = resolved.as_path();
        let length = output
            .as_os_str()
            .encode_wide()
            .take(MAX_PATH_UNITS + 1)
            .count();
        budget.charge(Resource::Steps, length as u64 * PATH_SCAN_PASSES)?;
        budget.charge(
            Resource::Bytes,
            (length * size_of::<u16>() * 2 + size_of::<u16>()) as u64,
        )?;
        if length > MAX_PATH_UNITS || !output.is_absolute() {
            return Err(invalid());
        }
        match output.components().next() {
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)) => {}
            _ => return Err(invalid()),
        }
        for component in output.components() {
            match component {
                Component::Normal(name) => {
                    let mut component = [0; MAX_COMPONENT_UNITS];
                    let mut count = 0;
                    for unit in name.encode_wide() {
                        if count == component.len() {
                            return Err(invalid());
                        }
                        component[count] = unit;
                        count += 1;
                    }
                    if !names::valid_component(&component[..count]) {
                        return Err(invalid());
                    }
                }
                Component::Prefix(_) | Component::RootDir => {}
                _ => return Err(invalid()),
            }
        }
        let name = bounded_wide(
            output.file_name().ok_or_else(invalid)?,
            MAX_COMPONENT_UNITS,
            false,
        )?;
        if !names::valid_component(&name) {
            return Err(invalid());
        }
        let parent_path = output.parent().ok_or_else(invalid)?;
        let path = bounded_wide(parent_path.as_os_str(), MAX_PATH_UNITS, true)?;
        let parent = open_parent(&path)?;
        qualify_local_ntfs(&parent)?;
        let identity = identity(&parent)?;
        Ok(Self {
            parent,
            name,
            identity,
        })
    }
}

fn bounded_wide(
    value: &std::ffi::OsStr,
    maximum: usize,
    terminated: bool,
) -> Result<Vec<u16>, IoError> {
    let length = value.encode_wide().take(maximum + 1).count();
    if length > maximum {
        return Err(invalid());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(length + usize::from(terminated))
        .map_err(|_| IoError::Io(ErrorKind::OutOfMemory))?;
    result.extend(value.encode_wide());
    if terminated {
        result.push(0);
    }
    Ok(result)
}
