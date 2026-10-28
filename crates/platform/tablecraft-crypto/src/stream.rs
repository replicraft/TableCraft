use crate::CryptoError;
use rc4::{KeyInit, StreamCipher};

/// One zeroizing RC4 stream, preserving position across explicit segments.
/// No Debug, Clone or serialization exposes or duplicates its key schedule.
pub struct Rc4Stream(rc4::Rc4);

impl Rc4Stream {
    /// Initialize a stream from a caller-owned key (1 through 256 bytes).
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        rc4::Rc4::new_from_slice(key)
            .map(Self)
            .map_err(|_| CryptoError::KeyLength)
    }

    /// Transform one caller-bounded segment and advance this same stream.
    /// Charging, cancellation and scratch/publication policy belong to caller.
    pub fn apply(&mut self, bytes: &mut [u8]) {
        self.0.apply_keystream(bytes);
    }
}
