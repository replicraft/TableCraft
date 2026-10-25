//! Decode only bytes requested by the schema event reader, with no chunk lookahead.
use super::{Encoding, MAX_XML_BYTES, detect, next};
use crate::template::{TemplateError, check};
use std::io::{self, BufRead, Read};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const PARSER_WORK_FACTOR: u64 = 8;
const SCALAR_BYTES: usize = 4;

pub(crate) struct PrefixReader<'a, 'b> {
    bytes: &'a [u8],
    encoding: Encoding,
    position: usize,
    pending_end: usize,
    scalar: [u8; SCALAR_BYTES],
    start: usize,
    length: usize,
    decoded: usize,
    budget: &'b mut ResourceBudget,
    cancelled: &'b dyn Fn() -> bool,
    failure: Option<TemplateError>,
}

impl<'a, 'b> PrefixReader<'a, 'b> {
    pub(in crate::template) fn new(
        bytes: &'a [u8],
        budget: &'b mut ResourceBudget,
        cancelled: &'b dyn Fn() -> bool,
    ) -> Self {
        let (encoding, position) = detect(bytes);
        Self {
            bytes,
            encoding,
            position,
            pending_end: position,
            scalar: [0; SCALAR_BYTES],
            start: 0,
            length: 0,
            decoded: 0,
            budget,
            cancelled,
            failure: None,
        }
    }

    pub(in crate::template) fn check(&mut self) -> Result<(), TemplateError> {
        check(self.budget, self.cancelled)
    }

    pub(in crate::template) fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Raw offset advances only after every UTF-8 byte of a scalar is consumed.
    pub(in crate::template) fn position(&self) -> usize {
        self.position
    }

    pub(in crate::template) fn take_error(&mut self) -> TemplateError {
        self.failure.take().unwrap_or(TemplateError::Malformed)
    }

    fn fill_scalar(&mut self) -> Result<(), TemplateError> {
        self.check()?;
        if self.position >= MAX_XML_BYTES {
            return Err(TemplateError::Unsupported);
        }
        let (character, end) = next(self.bytes, self.position, self.encoding)?;
        let width = character.len_utf8();
        if end > MAX_XML_BYTES || self.decoded + width > MAX_XML_BYTES {
            return Err(TemplateError::Unsupported);
        }
        self.budget
            .charge(Resource::Steps, (end - self.position) as u64)
            .map_err(InspectError::from)?;
        self.budget
            .charge(Resource::Bytes, width as u64 * PARSER_WORK_FACTOR)
            .map_err(InspectError::from)?;
        character.encode_utf8(&mut self.scalar);
        self.pending_end = end;
        self.start = 0;
        self.length = width;
        Ok(())
    }
}

impl BufRead for PrefixReader<'_, '_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.start == self.length && self.position < self.bytes.len() {
            if self.failure.is_none()
                && let Err(error) = self.fill_scalar()
            {
                self.failure = Some(error);
            }
            if self.failure.is_some() {
                return Err(io::Error::other("template XML decoding failed"));
            }
        }
        Ok(&self.scalar[self.start..self.length])
    }

    fn consume(&mut self, amount: usize) {
        let consumed = amount.min(self.length - self.start);
        self.start += consumed;
        self.decoded += consumed;
        if self.start == self.length {
            self.position = self.pending_end;
        }
    }
}

impl Read for PrefixReader<'_, '_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let bytes = self.fill_buf()?;
        let length = bytes.len().min(output.len());
        output[..length].copy_from_slice(&bytes[..length]);
        self.consume(length);
        Ok(length)
    }
}
