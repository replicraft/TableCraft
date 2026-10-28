//! Decoder callback accounting with original typed failures and no callback-held borrow.
use super::DeploymentError;
use crate::ReadError;
use std::{
    cell::RefCell,
    io::{self, Read, Seek, SeekFrom, Write},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Context<'a> {
    budget: RefCell<&'a mut ResourceBudget>,
    cancelled: &'a dyn Fn() -> bool,
    failure: RefCell<Option<ReadError>>,
}

impl<'a> Context<'a> {
    pub fn new(budget: &'a mut ResourceBudget, cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            budget: RefCell::new(budget),
            cancelled,
            failure: RefCell::new(None),
        }
    }

    pub fn check(&self) -> io::Result<()> {
        if self.failure.borrow().is_some() {
            return Err(io::ErrorKind::Other.into());
        }
        if (self.cancelled)() {
            return Err(self.fail(ReadError::Cancelled));
        }
        self.charge(Resource::Steps, 1)
    }

    pub fn charge(&self, resource: Resource, amount: u64) -> io::Result<()> {
        let result = self.budget.borrow_mut().charge(resource, amount);
        result.map_err(|error| self.fail(error.into()))
    }

    pub fn fail(&self, error: ReadError) -> io::Error {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() {
            *failure = Some(error);
        }
        io::ErrorKind::Other.into()
    }

    pub fn result<T>(&self, result: io::Result<T>) -> Result<T, DeploymentError> {
        match self.failure.borrow_mut().take() {
            Some(error) => Err(error.into()),
            None => result.map_err(|_| DeploymentError::MalformedPackage),
        }
    }

    pub fn host<T>(&self, result: io::Result<T>) -> io::Result<T> {
        match result {
            Ok(value) => {
                self.check()?;
                Ok(value)
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                self.check()?;
                Err(error)
            }
            Err(error) => Err(self.fail(error.into())),
        }
    }

    pub fn read_exact<R: Read>(&self, source: &mut R, mut bytes: &mut [u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            self.check()?;
            match source.read(bytes) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(count) => {
                    bytes = bytes
                        .get_mut(count..)
                        .ok_or_else(|| self.fail(ReadError::Malformed))?
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        self.check()
    }

    pub fn read_host_exact<R: Read>(&self, source: &mut R, mut bytes: &mut [u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            self.check()?;
            let result = source.read(bytes);
            match self.host(result) {
                Ok(0) => return Err(self.fail(ReadError::Io(io::ErrorKind::UnexpectedEof))),
                Ok(count) => {
                    bytes = bytes
                        .get_mut(count..)
                        .ok_or_else(|| self.fail(ReadError::Malformed))?
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        self.check()
    }

    pub fn flush<W: Write>(&self, target: &mut W) -> io::Result<()> {
        loop {
            self.check()?;
            let result = target.flush();
            match self.host(result) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }

    pub fn end<S: Seek>(&self, target: &mut S) -> io::Result<u64> {
        loop {
            self.check()?;
            let result = target.seek(SeekFrom::End(0));
            match self.host(result) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }

    pub fn write_exact<W: Write>(&self, target: &mut W, mut bytes: &[u8]) -> io::Result<()> {
        self.charge(Resource::Bytes, bytes.len() as u64)?;
        while !bytes.is_empty() {
            self.check()?;
            let result = target.write(bytes);
            match self.host(result) {
                Ok(0) => return Err(self.fail(ReadError::Io(io::ErrorKind::WriteZero))),
                Ok(count) => {
                    bytes = bytes
                        .get(count..)
                        .ok_or_else(|| self.fail(ReadError::Malformed))?
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        self.check()
    }

    pub fn seek<S: Seek>(&self, target: &mut S, position: u64) -> io::Result<()> {
        loop {
            self.check()?;
            let result = target.seek(SeekFrom::Start(position));
            match self.host(result) {
                Ok(actual) if actual == position => return Ok(()),
                Ok(_) => return Err(self.fail(ReadError::Malformed)),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

pub(super) struct Source<'a, 'b, R> {
    pub source: &'a mut R,
    pub context: &'a Context<'b>,
    pub end: u64,
    pub position: u64,
}

impl<R: Read> Read for Source<'_, '_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.context.check()?;
        let length = usize::try_from((self.end - self.position).min(bytes.len() as u64)).unwrap();
        self.context.charge(Resource::Bytes, length as u64)?;
        let result = self.source.read(&mut bytes[..length]);
        let count = self.context.host(result)?;
        if count > length {
            return Err(self.context.fail(ReadError::Malformed));
        }
        self.position += count as u64;
        Ok(count)
    }
}

impl<R: Seek> Seek for Source<'_, '_, R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let target = match from {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.end) + i128::from(offset),
        };
        if !(0..=i128::from(self.end)).contains(&target) {
            return Err(self.context.fail(ReadError::Malformed));
        }
        let target = target as u64;
        self.context.seek(self.source, target)?;
        self.position = target;
        Ok(target)
    }
}
