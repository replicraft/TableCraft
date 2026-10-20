use std::{
    fs::File,
    io::{self, IsTerminal, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_ace::{EncryptedSource, Header, InspectError, NativeReadSource, inspect_header};
use tablecraft_core::limits::ResourceBudget;

pub(crate) enum NativeFile {
    Plain(File),
    Encrypted(EncryptedSource<File>),
}

pub(crate) fn open_raw(path: &Path) -> Result<NativeFile, InspectError> {
    Ok(NativeFile::Plain(File::open(path)?))
}
pub(crate) fn open(
    path: &Path,
    password: Option<&str>,
    budget: &mut ResourceBudget,
) -> Result<NativeFile, InspectError> {
    let mut source = File::open(path)?;
    let header = inspect_header(&mut source, budget, &|| false)?;
    if header.encoded_pages() {
        // Validate the descriptor before requesting credentials. Unsupported or
        // malformed encryption must not solicit a password it cannot consume.
        let prompted = if password.is_none() && io::stdin().is_terminal() {
            match EncryptedSource::open(&mut source, None, budget, &|| false) {
                Ok(_) => None,
                Err(InspectError::PasswordRequired) => Some(zeroize::Zeroizing::new(
                    rpassword::prompt_password("Database password: ")
                        .map_err(|_| InspectError::PasswordRequired)?,
                )),
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let password = password.or_else(|| prompted.as_ref().map(|p| p.as_str()));
        Ok(NativeFile::Encrypted(EncryptedSource::open(
            source,
            password,
            budget,
            &|| false,
        )?))
    } else {
        Ok(NativeFile::Plain(source))
    }
}
impl Read for NativeFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(bytes),
            Self::Encrypted(s) => s.read(bytes),
        }
    }
}
impl Seek for NativeFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Plain(s) => s.seek(from),
            Self::Encrypted(s) => s.seek(from),
        }
    }
}
impl NativeReadSource for NativeFile {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        match self {
            Self::Plain(s) => s.qualify_header(header),
            Self::Encrypted(s) => s.qualify_header(header),
        }
    }
    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        match self {
            Self::Plain(s) => s.decode_native_page(number, page, budget, cancelled),
            Self::Encrypted(s) => s.decode_native_page(number, page, budget, cancelled),
        }
    }
}
