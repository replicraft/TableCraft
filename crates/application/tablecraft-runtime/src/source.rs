//! Raw package selection and explicit authenticated native page decoding.
use super::Error;
use bytes::Bytes;
use std::io::{Cursor, Read, Seek, SeekFrom};
use tablecraft_access::AccessInput;
use tablecraft_ace::{EncryptedSource, Header, InspectError, NativeReadSource, inspect_header};
use tablecraft_core::limits::ResourceBudget;

type Input = AccessInput<Cursor<Bytes>>;

pub(super) enum Source {
    Plain {
        input: Input,
        #[cfg(all(
            feature = "offline-edit",
            any(target_os = "macos", target_os = "linux", windows)
        ))]
        fits_standard_image: bool,
    },
    Encrypted(EncryptedSource<Input>),
}

impl Read for Source {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain { input, .. } => input.read(buffer),
            Self::Encrypted(source) => source.read(buffer),
        }
    }
}

impl Seek for Source {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        match self {
            Self::Plain { input, .. } => input.seek(position),
            Self::Encrypted(source) => source.seek(position),
        }
    }
}

impl NativeReadSource for Source {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        match self {
            Self::Plain { input, .. } => input.qualify_header(header),
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
            Self::Plain { input, .. } => input.decode_native_page(number, page, budget, cancelled),
            Self::Encrypted(source) => source.decode_native_page(number, page, budget, cancelled),
        }
    }
}

impl Source {
    #[cfg(all(
        feature = "offline-edit",
        any(target_os = "macos", target_os = "linux", windows)
    ))]
    pub(super) fn editable_native(&self) -> bool {
        matches!(self, Self::Plain { input, fits_standard_image: true } if !input.is_deployment())
    }

    /// New retained actions qualify the actual image instead of inheriting the
    /// standard capture-size observation. Existing editing routes stay separate.
    #[cfg(all(
        feature = "offline-edit",
        feature = "ace16-create",
        any(target_os = "macos", target_os = "linux", windows)
    ))]
    pub(super) fn retained_native(&self) -> bool {
        matches!(self, Self::Plain { input, .. } if !input.is_deployment())
    }

    pub(super) fn open(
        bytes: Bytes,
        password: &str,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, String), Error> {
        #[cfg(all(
            feature = "offline-edit",
            any(target_os = "macos", target_os = "linux", windows)
        ))]
        let fits_standard_image =
            bytes.len() as u64 <= tablecraft_ace::NativeImage::STANDARD_MAX_BYTES;
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
            Source::Plain {
                input,
                #[cfg(all(
                    feature = "offline-edit",
                    any(target_os = "macos", target_os = "linux", windows)
                ))]
                fits_standard_image,
            }
        };
        Ok((source, format))
    }
}
