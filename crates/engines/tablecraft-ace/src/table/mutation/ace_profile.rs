//! Common read/source selector checks, independent of candidate packaging.
use super::{EditError, WriteBlocker};
use crate::{FileFormat, NativeImage};

const SUPPORTED_SUBVERSION: u8 = 0;

pub(super) fn qualify(image: &NativeImage) -> Result<(), EditError> {
    image.require_standard_capture()?;
    qualify_format(image)
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn qualify_fixed(image: &NativeImage) -> Result<(), EditError> {
    image.require_fixed_update_capture()?;
    qualify_format(image)
}

fn qualify_format(image: &NativeImage) -> Result<(), EditError> {
    if !matches!(
        image.catalog().header().format(),
        FileFormat::Ace2016 | FileFormat::Ace2019
    ) || image.catalog().header().subversion() != SUPPORTED_SUBVERSION
        || image.catalog().header().encoded_pages()
    {
        return Err(EditError::Unsupported(WriteBlocker::Format));
    }
    Ok(())
}
