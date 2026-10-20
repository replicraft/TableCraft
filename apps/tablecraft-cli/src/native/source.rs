use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_ace::{AgileSource, Header, InspectError, NativeReadSource, inspect_header};
use tablecraft_core::limits::ResourceBudget;

pub(crate) enum NativeFile {
    Plain(File),
    Agile(AgileSource<File>),
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
        Ok(NativeFile::Agile(AgileSource::open(
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
            Self::Agile(s) => s.read(bytes),
        }
    }
}
impl Seek for NativeFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Plain(s) => s.seek(from),
            Self::Agile(s) => s.seek(from),
        }
    }
}
impl NativeReadSource for NativeFile {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        match self {
            Self::Plain(s) => s.qualify_header(header),
            Self::Agile(s) => s.qualify_header(header),
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
            Self::Agile(s) => s.decode_native_page(number, page, budget, cancelled),
        }
    }
}
