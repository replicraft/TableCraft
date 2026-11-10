use crate::{UpdateError, release::MAX_SIGNATURE_BYTES};
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

const TRANSFER_BYTES: usize = 64 * 1024;

pub(crate) fn signature(
    text: &[u8],
    version: &Version,
    asset_name: &str,
) -> Result<Signature, UpdateError> {
    if text.len() as u64 > MAX_SIGNATURE_BYTES {
        return Err(UpdateError::AssetTooLarge);
    }
    let text = str::from_utf8(text).map_err(|_| UpdateError::Signature)?;
    if text.lines().count() != 4 {
        return Err(UpdateError::Signature);
    }
    let signature = Signature::decode(text).map_err(|_| UpdateError::Signature)?;
    let expected = format!("tablecraft-update {version} {asset_name}");
    if signature.trusted_comment() != expected {
        return Err(UpdateError::Signature);
    }
    Ok(signature)
}

pub(crate) fn transfer(
    key: &PublicKey,
    signature: &Signature,
    source: &mut dyn Read,
    destination: &mut dyn Write,
    expected: u64,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<[u8; 32], UpdateError> {
    // Streaming verification refuses legacy un-prehashed Minisign signatures.
    let mut verifier = key
        .verify_stream(signature)
        .map_err(|_| UpdateError::Signature)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; TRANSFER_BYTES];
    let mut received = 0;

    loop {
        if !progress(received) {
            return Err(UpdateError::Cancelled);
        }
        // Always read one byte beyond the advertised remaining length to catch
        // appended bytes, including when Content-Length was absent or inaccurate.
        let allowance = usize::try_from((expected - received).min(TRANSFER_BYTES as u64 - 1) + 1)
            .map_err(|_| UpdateError::AssetTooLarge)?;
        let count = source
            .read(&mut buffer[..allowance])
            .map_err(UpdateError::Io)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > expected {
            return Err(UpdateError::LengthMismatch);
        }
        let bytes = &buffer[..count];
        verifier.update(bytes);
        digest.update(bytes);
        destination.write_all(bytes).map_err(UpdateError::Io)?;
    }

    if received != expected {
        return Err(UpdateError::LengthMismatch);
    }
    verifier.finalize().map_err(|_| UpdateError::Signature)?;
    Ok(digest.finalize().into())
}

pub(crate) fn bounded_bytes(
    source: &mut dyn Read,
    limit: usize,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<u8>, UpdateError> {
    let mut bytes = Vec::new();
    let mut buffer = [0; TRANSFER_BYTES];
    loop {
        if cancelled() {
            return Err(UpdateError::Cancelled);
        }
        let allowance = (limit - bytes.len()).min(buffer.len() - 1) + 1;
        let count = source
            .read(&mut buffer[..allowance])
            .map_err(UpdateError::Io)?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes.len() + count > limit {
            return Err(UpdateError::AssetTooLarge);
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}
