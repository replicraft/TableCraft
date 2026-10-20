//! Native header masking and SID-key folding; not password encryption.
use crate::InspectError;
use crate::format::header_mask::{self, PREFIX_END};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const PASSWORD_OFFSET: usize = 66;
const PASSWORD_BYTES: usize = 40;
const DATE_OFFSET: usize = 114;
const SID_SHIFT_PERIOD: usize = 24;

pub(super) struct NativeHeader(Zeroizing<[u8; PREFIX_END]>);
impl NativeHeader {
    pub(super) fn read(page: &[u8]) -> Result<Self, InspectError> {
        let mut bytes = Zeroizing::new([0; PREFIX_END]);
        let length = bytes.len();
        bytes.copy_from_slice(page.get(..length).ok_or(InspectError::Truncated)?);
        header_mask::apply(&mut *bytes)?;
        Ok(Self(bytes))
    }
    pub(super) fn write(&self, page: &mut [u8]) -> Result<(), InspectError> {
        let mut bytes = Zeroizing::new(*self.0);
        header_mask::apply(&mut *bytes)?;
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
    pub(super) fn office_key(&mut self, key: [u8; 4]) -> Result<(), InspectError> {
        self.encoding_key(key);
        let mask = self.password_mask()?;
        for (i, byte) in self.0[PASSWORD_OFFSET..PASSWORD_OFFSET + PASSWORD_BYTES]
            .iter_mut()
            .enumerate()
        {
            *byte = key[0] ^ mask[i % mask.len()];
        }
        if self.0[crate::header::FORMAT_CODE_OFFSET] < crate::header::ACE2010_CODE {
            self.0[crate::header::FORMAT_CODE_OFFSET] = crate::header::ACE2010_CODE;
            self.0[crate::header::SUBVERSION_OFFSET] = 1;
        }
        Ok(())
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
