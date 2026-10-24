use super::{
    AttachmentDecodeLimits, AttachmentDecoder, AttachmentEncoding, DecodedAttachment, inflate,
};
use crate::{
    InspectError,
    format::{bytes::u32_at, text},
    header::check,
};
use flate2::Decompress;
use tablecraft_core::limits::{Resource, ResourceBudget};

const ENCODING_OFFSET: usize = 0;
const LENGTH_OFFSET: usize = size_of::<u32>();
const OUTER_BYTES: usize = LENGTH_OFFSET + size_of::<u32>();
const HEADER_LENGTH_OFFSET: usize = 0;
const HEADER_TAG_OFFSET: usize = size_of::<u32>();
const TYPE_UNITS_OFFSET: usize = HEADER_TAG_OFFSET + size_of::<u32>();
const HEADER_BYTES: usize = TYPE_UNITS_OFFSET + size_of::<u32>();
const SUPPORTED_HEADER_TAG: u32 = 1;
const UTF16_BYTES: usize = size_of::<u16>();
const MAX_HEADER_BYTES: usize = 4096;

/// Decode an explicitly supplied native FileData attachment wrapper without
/// opening content or writing files. Encoded inputs are capped at 2 MiB,
/// decoded bodies at 1 MiB plus 4 KiB, type headers at 4 KiB and files at 1 MiB.
/// Unknown encodings/header variants are unsupported; malformed lengths, invalid
/// zlib/checksums, trailing streams and excess output fail without partial data.
/// Budgets/cancellation apply between bounded copy/codec blocks. The selected
/// codec's internal allocations have a logical allowance, not allocator control.
pub fn decode_attachment(
    encoded: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DecodedAttachment, InspectError> {
    AttachmentDecoder::new(budget, cancelled).decode(encoded)
}

/// Decode with an explicit file ceiling while retaining encoded/header bounds.
/// A larger allowance does not waive checksum, exact-length or resource checks.
pub fn decode_attachment_with_limits(
    encoded: &[u8],
    limits: AttachmentDecodeLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DecodedAttachment, InspectError> {
    AttachmentDecoder::new_with_limits(budget, cancelled, limits).decode(encoded)
}

pub(super) fn using_inflater(
    encoded: &[u8],
    inflater: &mut Option<Decompress>,
    limits: AttachmentDecodeLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DecodedAttachment, InspectError> {
    let _span =
        tracing::debug_span!("native.attachment.decode", encoded_bytes = encoded.len()).entered();
    check(budget, cancelled)?;
    if encoded.len() > limits.maximum_encoded_bytes() as usize {
        return Err(InspectError::StructureLimit);
    }
    if encoded.len() < OUTER_BYTES {
        return Err(InspectError::Truncated);
    }
    let encoding = match u32_at(encoded, ENCODING_OFFSET)? {
        0 => AttachmentEncoding::Uncompressed,
        1 => AttachmentEncoding::Zlib,
        _ => return Err(InspectError::Unsupported),
    };
    let length = u32_at(encoded, LENGTH_OFFSET)? as usize;
    if length > MAX_HEADER_BYTES + limits.maximum_file_bytes() as usize {
        return Err(InspectError::StructureLimit);
    }
    if length < HEADER_BYTES + UTF16_BYTES {
        return Err(InspectError::Malformed);
    }
    let source = &encoded[OUTER_BYTES..];
    let body = match encoding {
        AttachmentEncoding::Zlib => inflate::zlib(source, length, inflater, budget, cancelled)?,
        AttachmentEncoding::Uncompressed => {
            if source.len() != length {
                return Err(InspectError::Malformed);
            }
            budget.charge_allocation(length as u64, 1)?;
            let mut body = Vec::new();
            body.try_reserve_exact(length)
                .map_err(|_| InspectError::Allocation)?;
            for block in source.chunks(inflate::BLOCK_BYTES) {
                check(budget, cancelled)?;
                budget.charge(Resource::Steps, block.len() as u64)?;
                body.extend_from_slice(block);
            }
            body
        }
    };
    let header = u32_at(&body, HEADER_LENGTH_OFFSET)? as usize;
    let units = u32_at(&body, TYPE_UNITS_OFFSET)? as usize;
    if u32_at(&body, HEADER_TAG_OFFSET)? != SUPPORTED_HEADER_TAG {
        return Err(InspectError::Unsupported);
    }
    if !(HEADER_BYTES + UTF16_BYTES..=MAX_HEADER_BYTES).contains(&header)
        || header > body.len()
        || units == 0
        || units > (MAX_HEADER_BYTES - HEADER_BYTES) / UTF16_BYTES
        || header != HEADER_BYTES + units * UTF16_BYTES
        || body[header - UTF16_BYTES..header] != [0, 0]
    {
        return Err(InspectError::Malformed);
    }
    if body.len() - header > limits.maximum_file_bytes() as usize {
        return Err(InspectError::StructureLimit);
    }
    let type_label = text::decode(&body[HEADER_BYTES..header - UTF16_BYTES], false, budget)?;
    check(budget, cancelled)?;
    Ok(DecodedAttachment {
        body,
        data_start: header,
        type_label,
        encoding,
    })
}
