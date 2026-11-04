//! Raw package selection and explicit authenticated native page decoding.
use super::Error;
use std::io::{Cursor, Read, Seek, SeekFrom};
use tablecraft_access::AccessInput;
use tablecraft_ace::{EncryptedSource, Header, InspectError, NativeReadSource, inspect_header};
use tablecraft_core::limits::ResourceBudget;

type Input = AccessInput<Cursor<Vec<u8>>>;

pub(super) enum Source {
    Plain(Input),
    Encrypted(EncryptedSource<Input>),
}

impl Read for Source {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(source) => source.read(buffer),
            Self::Encrypted(source) => source.read(buffer),
        }
    }
}

impl Seek for Source {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::Plain(source) => source.seek(position),
            Self::Encrypted(source) => source.seek(position),
        }
    }
}

impl NativeReadSource for Source {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        match self {
            Self::Plain(source) => source.qualify_header(header),
            Self::Encrypted(source) => source.qualify_header(header),
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
            Self::Plain(source) => source.decode_native_page(number, page, budget, cancelled),
            Self::Encrypted(source) => source.decode_native_page(number, page, budget, cancelled),
        }
    }
}

impl Source {
    pub(super) fn open(
        bytes: Vec<u8>,
        password: &str,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, String), Error> {
        let mut input = AccessInput::open(Cursor::new(bytes), work, cancelled)?;
        let deployment = input.is_deployment();
        let header = inspect_header(&mut input, work, cancelled)?;
        let format = format!(
            "{:?}{}",
            header.format(),
            if deployment {
                " · deployment package"
            } else {
                ""
            }
        );
        let source = if header.encoded_pages() {
            let password = (!password.is_empty()).then_some(password);
            Source::Encrypted(EncryptedSource::open(input, password, work, cancelled)?)
        } else {
            Source::Plain(input)
        };
        Ok((source, format))
    }
}
