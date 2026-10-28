//! Qualify decoded CAB payloads as native Access databases before publication.
use super::AccessInputError;
use std::io::{Read, Seek, Write};
use tablecraft_ace::{FileFormat, Header, InspectError};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_formats::{
    ReadError,
    deployment::{self, DeploymentError, Selected, SpooledPayload},
};

pub(super) fn read<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AccessInputError> {
    deployment::read_validated(source, budget, cancelled, |bytes| {
        qualify(Header::parse(bytes, bytes.len() as u64)?)
    })
}

pub(super) fn read_spooled<R, S, F>(
    source: &mut R,
    make_spool: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<S, AccessInputError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, InspectError>,
{
    let mut factory_error = None;
    let result = deployment::read_spooled(
        source,
        || {
            make_spool().map_err(|error| {
                factory_error = Some(error);
                ReadError::Malformed
            })
        },
        budget,
        cancelled,
    );
    if let Some(error) = factory_error {
        return Err(error.into());
    }
    qualify_spool(result.map_err(AccessInputError::from)?, budget, cancelled)
}

pub(super) fn read_with_spool<R, S, F>(
    source: &mut R,
    make_spool: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Selected<S>, AccessInputError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, InspectError>,
{
    let mut factory_error = None;
    let result = deployment::read_with_spool_validated(
        source,
        || {
            make_spool().map_err(|error| {
                factory_error = Some(error);
                ReadError::Malformed
            })
        },
        budget,
        cancelled,
        |bytes| qualify(Header::parse(bytes, bytes.len() as u64)?),
    );
    if let Some(error) = factory_error {
        return Err(error.into());
    }
    let mut selected = result?;
    if let Selected::Spooled(spool) = &mut selected {
        let header = Header::parse(spool.prefix(), spool.length())?;
        spool
            .rewind(budget, cancelled)
            .map_err(AccessInputError::from)?;
        qualify(header)?;
    }
    Ok(selected)
}

fn qualify_spool<S: Seek>(
    mut payload: SpooledPayload<S>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<S, AccessInputError> {
    let header = Header::parse(payload.prefix(), payload.length())?;
    payload
        .rewind(budget, cancelled)
        .map_err(AccessInputError::from)?;
    qualify(header)?;
    Ok(payload.into_inner())
}

fn qualify(header: Header) -> Result<(), AccessInputError> {
    if !matches!(
        header.format(),
        FileFormat::Ace2007 | FileFormat::Ace2010 | FileFormat::Ace2016 | FileFormat::Ace2019
    ) {
        return Err(AccessInputError::UnsupportedPackage);
    }
    Ok(())
}

impl From<DeploymentError> for AccessInputError {
    fn from(error: DeploymentError) -> Self {
        match error {
            DeploymentError::MalformedPackage => Self::MalformedPackage,
            DeploymentError::UnsupportedPackage => Self::UnsupportedPackage,
            DeploymentError::Read(error) => native_error(error).into(),
        }
    }
}

fn native_error(error: ReadError) -> InspectError {
    match error {
        ReadError::Io(kind) => InspectError::Io(kind),
        ReadError::Truncated => InspectError::Truncated,
        ReadError::Malformed => InspectError::Malformed,
        ReadError::Cancelled => InspectError::Cancelled,
        ReadError::Limit(error) => InspectError::Limit(error),
        ReadError::Allocation => InspectError::Allocation,
        ReadError::StructureLimit => InspectError::StructureLimit,
        ReadError::LockBusy => InspectError::LockBusy,
    }
}
