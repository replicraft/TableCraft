//! Apply the same explicit scalar/collection policy to root, linked and external sources.
use super::{Error, retrieval::NativeInput};
use tablecraft_access::NativeSource;
use tablecraft_ace::{AttachmentDecodeLimits, LongValueReadLimits, NativeReadSource};

pub(super) fn configure<'a, R: NativeReadSource>(
    source: NativeSource<'a, R>,
    input: &NativeInput<'_>,
    value_limits: LongValueReadLimits,
) -> Result<NativeSource<'a, R>, Error> {
    let source = if input.stored_calculations {
        source.with_stored_calculations().map_err(Error::Native)?
    } else {
        source
    };
    let source = if input.auto_index {
        source.with_auto_indexes()
    } else {
        source
    };

    Ok(if input.decoded_attachments {
        let limits = input
            .attachment_bytes
            .map(AttachmentDecodeLimits::new)
            .transpose()
            .map_err(Error::Native)?
            .unwrap_or_default()
            .with_encoded_bytes(value_limits.maximum_bytes())
            .map_err(Error::Native)?;
        source.with_decoded_attachment_limits(limits)
    } else if input.collections {
        source.with_collections()
    } else if input.multivalues {
        source.with_multivalues()
    } else {
        source
    })
}
