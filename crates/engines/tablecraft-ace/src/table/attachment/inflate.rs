use crate::{InspectError, header::check};
use flate2::{Decompress, FlushDecompress, Status};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const BLOCK_BYTES: usize = 4096;
// Logical allowance for the selected miniz_oxide boxed inflater/dictionary.
// Upstream internal allocations are not intercepted by ResourceBudget.
const CODEC_WORKSPACE_BYTES: u64 = 128 * 1024;

pub(super) fn zlib(
    input: &[u8],
    length: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    check(budget, cancelled)?;
    budget.charge(Resource::Bytes, CODEC_WORKSPACE_BYTES)?;
    // One sentinel byte detects output beyond the declared body length, while
    // allowing checksum/end-marker consumption after the expected bytes.
    budget.charge_allocation((length + 1) as u64, 1)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length + 1)
        .map_err(|_| InspectError::Allocation)?;
    while output.len() < length + 1 {
        check(budget, cancelled)?;
        let end = (output.len() + BLOCK_BYTES).min(length + 1);
        budget.charge(Resource::Steps, (end - output.len()) as u64)?;
        output.resize(end, 0);
    }
    let mut stream = Decompress::new(true);
    loop {
        check(budget, cancelled)?;
        let before = (stream.total_in(), stream.total_out());
        let read = usize::try_from(before.0).map_err(|_| InspectError::Malformed)?;
        let written = usize::try_from(before.1).map_err(|_| InspectError::Malformed)?;
        if read > input.len() || written > length {
            return Err(InspectError::Malformed);
        }
        let source = &input[read..(read + BLOCK_BYTES).min(input.len())];
        let end = (written + BLOCK_BYTES).min(output.len());
        budget.charge(Resource::Steps, (source.len() + end - written) as u64)?;
        let status = stream
            .decompress(source, &mut output[written..end], FlushDecompress::None)
            .map_err(|error| {
                if error.needs_dictionary().is_some() {
                    InspectError::Unsupported
                } else {
                    InspectError::Malformed
                }
            })?;
        if stream.total_out() > length as u64 {
            return Err(InspectError::Malformed);
        }
        if status == Status::StreamEnd {
            if stream.total_in() != input.len() as u64 || stream.total_out() != length as u64 {
                return Err(InspectError::Malformed);
            }
            check(budget, cancelled)?;
            output.truncate(length);
            return Ok(output);
        }
        if before == (stream.total_in(), stream.total_out()) {
            return Err(InspectError::Malformed);
        }
    }
}
