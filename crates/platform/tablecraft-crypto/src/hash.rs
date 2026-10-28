use sha2::Digest;
use zeroize::{Zeroize, Zeroizing};

/// Largest output of the closed digest algorithm set.
pub const MAX_DIGEST_BYTES: usize = 64;

/// Explicit algorithms consumed by compatibility codecs; no default is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashAlgorithm {
    /// MD5.
    Md5,
    /// SHA-1.
    Sha1,
    /// SHA-256.
    Sha256,
    /// SHA-384.
    Sha384,
    /// SHA-512.
    Sha512,
}

/// Hash caller-bounded parts into fixed zeroizing storage, with zero tail bytes.
/// The consumer must charge and bound all input work before calling. No salt,
/// password encoding, spin count, output truncation or key expansion is chosen.
pub fn digest(algorithm: HashAlgorithm, parts: &[&[u8]]) -> Zeroizing<[u8; MAX_DIGEST_BYTES]> {
    let mut output = Zeroizing::new([0; MAX_DIGEST_BYTES]);
    match algorithm {
        HashAlgorithm::Md5 => run::<md5::Md5>(parts, &mut output),
        HashAlgorithm::Sha1 => run::<sha1::Sha1>(parts, &mut output),
        HashAlgorithm::Sha256 => run::<sha2::Sha256>(parts, &mut output),
        HashAlgorithm::Sha384 => run::<sha2::Sha384>(parts, &mut output),
        HashAlgorithm::Sha512 => run::<sha2::Sha512>(parts, &mut output),
    }
    output
}

fn run<D: Digest>(parts: &[&[u8]], output: &mut [u8; MAX_DIGEST_BYTES]) {
    let mut digest = D::new();
    for part in parts {
        digest.update(part);
    }
    let mut value = digest.finalize();
    output[..value.len()].copy_from_slice(&value);
    value.as_mut_slice().zeroize();
}
