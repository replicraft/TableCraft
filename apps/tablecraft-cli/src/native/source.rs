use std::{
    fs::File,
    io::{self, IsTerminal, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_access::{AccessInput, AccessInputError};
use tablecraft_ace::{EncryptedSource, Header, InspectError, NativeReadSource, inspect_header};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::SharedFile;

pub(crate) struct NativeFile {
    inner: Kind,
    deployment: bool,
}
enum Kind {
    Plain(AccessInput<SharedFile, File>),
    Encrypted(EncryptedSource<AccessInput<SharedFile, File>>),
}
impl NativeFile {
    pub(crate) fn is_deployment(&self) -> bool {
        self.deployment
    }

    /// Release the original native source guard before publishing an output.
    /// Deployment inputs have no recoverable native resource and are refused.
    pub(crate) fn close(self) -> Result<(), AccessInputError> {
        let source = match self.inner {
            Kind::Plain(source) => source,
            Kind::Encrypted(source) => source.into_inner(),
        };
        let source = source
            .into_native()
            .map_err(|_| AccessInputError::UnsupportedPackage)?;
        source
            .close()
            .map_err(InspectError::from)
            .map_err(Into::into)
    }
}

pub(crate) fn open_raw(
    path: &Path,
    budget: &mut ResourceBudget,
) -> Result<NativeFile, AccessInputError> {
    let source = AccessInput::open_with_spool(
        SharedFile::open(path).map_err(InspectError::from)?,
        || tempfile::tempfile().map_err(InspectError::from),
        budget,
        &|| false,
    )?;
    let deployment = source.is_deployment();
    Ok(NativeFile {
        inner: Kind::Plain(source),
        deployment,
    })
}
pub(crate) fn open(
    path: &Path,
    password: Option<&str>,
    budget: &mut ResourceBudget,
) -> Result<NativeFile, AccessInputError> {
    let raw = open_raw(path, budget)?;
    let deployment = raw.deployment;
    let Kind::Plain(mut source) = raw.inner else {
        unreachable!()
    };
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
                Err(error) => return Err(error.into()),
            }
        } else {
            None
        };
        let password = password.or_else(|| prompted.as_ref().map(|p| p.as_str()));
        Ok(NativeFile {
            inner: Kind::Encrypted(EncryptedSource::open(source, password, budget, &|| false)?),
            deployment,
        })
    } else {
        Ok(NativeFile {
            inner: Kind::Plain(source),
            deployment,
        })
    }
}
impl Read for NativeFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            Kind::Plain(s) => s.read(bytes),
            Kind::Encrypted(s) => s.read(bytes),
        }
    }
}
impl Seek for NativeFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        match &mut self.inner {
            Kind::Plain(s) => s.seek(from),
            Kind::Encrypted(s) => s.seek(from),
        }
    }
}
impl NativeReadSource for NativeFile {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        match &self.inner {
            Kind::Plain(s) => s.qualify_header(header),
            Kind::Encrypted(s) => s.qualify_header(header),
        }
    }
    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        match &mut self.inner {
            Kind::Plain(s) => s.decode_native_page(number, page, budget, cancelled),
            Kind::Encrypted(s) => s.decode_native_page(number, page, budget, cancelled),
        }
    }
}
