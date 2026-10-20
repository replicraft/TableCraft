//! Native header masking and SID-key folding; not password encryption.
use crate::InspectError;
use rc4::{KeyInit, StreamCipher};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const HEADER_MASK_START: usize = 24;
const HEADER_MASK_BYTES: usize = 128;
const HEADER_MASK_KEY: [u8; 4] = [0xc7, 0xda, 0x39, 0x6b];
const PASSWORD_OFFSET: usize = 66;
const PASSWORD_BYTES: usize = 40;
const DATE_OFFSET: usize = 114;
const SID_SHIFT_PERIOD: usize = 24;

pub(super) struct NativeHeader(Zeroizing<[u8; HEADER_MASK_START + HEADER_MASK_BYTES]>);
impl NativeHeader {
    pub(super) fn read(page: &[u8]) -> Result<Self, InspectError> {
        let mut bytes = Zeroizing::new([0; HEADER_MASK_START + HEADER_MASK_BYTES]);
        let length = bytes.len();
        bytes.copy_from_slice(page.get(..length).ok_or(InspectError::Truncated)?);
        rc4::Rc4::new_from_slice(&HEADER_MASK_KEY)
            .map_err(|_| InspectError::MalformedEncryption)?
            .apply_keystream(&mut bytes[HEADER_MASK_START..]);
        Ok(Self(bytes))
    }
    pub(super) fn write(&self, page: &mut [u8]) -> Result<(), InspectError> {
        let mut bytes = Zeroizing::new(*self.0);
        rc4::Rc4::new_from_slice(&HEADER_MASK_KEY)
            .map_err(|_| InspectError::MalformedEncryption)?
            .apply_keystream(&mut bytes[HEADER_MASK_START..]);
        page.get_mut(..bytes.len())
            .ok_or(InspectError::Truncated)?
            .copy_from_slice(&*bytes);
        Ok(())
    }
    fn password_mask(&self) -> Result<Zeroizing<[u8; 4]>, InspectError> {
        let date = f64::from_le_bytes(
            self.0[DATE_OFFSET..DATE_OFFSET + size_of::<f64>()]
                .try_into()
                .unwrap(),
        );
        if !date.is_finite() || date < f64::from(i32::MIN) || date > f64::from(i32::MAX) {
            return Err(InspectError::Malformed);
        }
        Ok(Zeroizing::new((date as i32).to_le_bytes()))
    }
    pub(super) fn sid_key(&self) -> Result<Zeroizing<[u8; 4]>, InspectError> {
        let mask = self.password_mask()?;
        let mut region = Zeroizing::new([0; PASSWORD_BYTES * 2]);
        region.copy_from_slice(&self.0[PASSWORD_OFFSET..PASSWORD_OFFSET + PASSWORD_BYTES * 2]);
        for (i, byte) in region[..PASSWORD_BYTES].iter_mut().enumerate() {
            *byte ^= mask[i % mask.len()];
        }
        let mut key = u32::from_le_bytes(
            self.0[DATE_OFFSET..DATE_OFFSET + size_of::<u32>()]
                .try_into()
                .unwrap(),
        );
        for i in 0..PASSWORD_BYTES {
            key ^= u32::from(region[i * 2]) << (i % SID_SHIFT_PERIOD);
        }
        Ok(Zeroizing::new(key.to_le_bytes()))
    }
    pub(super) fn clear(&mut self) -> Result<(), InspectError> {
        self.encoding_key([0; 4]);
        let mask = self.password_mask()?;
        for (i, byte) in self.0[PASSWORD_OFFSET..PASSWORD_OFFSET + PASSWORD_BYTES]
            .iter_mut()
            .enumerate()
        {
            *byte = mask[i % mask.len()];
        }
        Ok(())
    }
    pub(super) fn encoding_key(&mut self, key: [u8; 4]) {
        self.0[crate::header::ENCODING_KEY_RANGE].copy_from_slice(&key);
    }
    pub(super) fn has_password(&self) -> Result<bool, InspectError> {
        let mask = self.password_mask()?;
        Ok(
            (self.0[PASSWORD_OFFSET] ^ mask[0]) != 0
                || (self.0[PASSWORD_OFFSET + 1] ^ mask[1]) != 0,
        )
    }
    pub(super) fn verify_jet_password(&self, password: Option<&str>) -> Result<(), InspectError> {
        let mask = self.password_mask()?;
        let mut expected = Zeroizing::new([0; PASSWORD_BYTES]);
        for (i, byte) in expected.iter_mut().enumerate() {
            *byte = self.0[PASSWORD_OFFSET + i] ^ mask[i % mask.len()];
        }
        let length = expected
            .as_chunks::<2>()
            .0
            .iter()
            .position(|u| *u == [0, 0])
            .unwrap_or(PASSWORD_BYTES / 2)
            * 2;
        if length == 0 {
            return Ok(());
        }
        let password = password.ok_or(InspectError::PasswordRequired)?;
        let mut actual = Zeroizing::new([0; PASSWORD_BYTES]);
        let mut used = 0;
        for unit in password.encode_utf16() {
            if used == actual.len() {
                return Err(InspectError::InvalidPasswordLength);
            }
            actual[used..used + size_of::<u16>()].copy_from_slice(&unit.to_le_bytes());
            used += size_of::<u16>();
        }
        expected[length..].fill(0);
        if used != length || !bool::from(expected[..].ct_eq(&actual[..])) {
            return Err(InspectError::IncorrectPassword);
        }
        Ok(())
    }
    pub(super) fn jet_password(&mut self, password: &str) -> Result<(), InspectError> {
        if password.contains('\0') {
            return Err(InspectError::UnsupportedPassword);
        }
        let mask = self.password_mask()?;
        let mut bytes = Zeroizing::new([0; PASSWORD_BYTES]);
        let mut used = 0;
        for unit in password.encode_utf16() {
            if used == bytes.len() {
                return Err(InspectError::InvalidPasswordLength);
            }
            bytes[used..used + size_of::<u16>()].copy_from_slice(&unit.to_le_bytes());
            used += size_of::<u16>();
        }
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte ^= mask[i % mask.len()];
        }
        self.0[PASSWORD_OFFSET..PASSWORD_OFFSET + PASSWORD_BYTES].copy_from_slice(&*bytes);
        Ok(())
    }
}
