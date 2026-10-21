use crate::{InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(crate) struct Work<'a> {
    pub budget: &'a mut ResourceBudget,
    pub cancelled: &'a dyn Fn() -> bool,
}
impl Work<'_> {
    pub fn tick(&mut self) -> Result<(), InspectError> {
        check(self.budget, self.cancelled)
    }
    pub fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>, InspectError> {
        self.tick()?;
        self.budget
            .charge_allocation(capacity as u64, size_of::<T>() as u64)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(capacity)
            .map_err(|_| InspectError::Allocation)?;
        Ok(result)
    }
}
pub(super) struct Buffer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Buffer {
    pub fn new(capacity: usize, work: &mut Work<'_>) -> Result<Self, InspectError> {
        Ok(Self {
            bytes: work.vector(capacity)?,
            limit: capacity,
        })
    }
    pub fn append(&mut self, bytes: &[u8]) -> Result<(), InspectError> {
        if self.bytes.len() + bytes.len() > self.limit {
            return Err(InspectError::StructureLimit);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub fn push(&mut self, byte: u8) -> Result<(), InspectError> {
        self.append(&[byte])
    }
    pub fn fill(&mut self, count: usize, byte: u8) -> Result<(), InspectError> {
        if self.bytes.len() + count > self.limit {
            return Err(InspectError::StructureLimit);
        }
        self.bytes.resize(self.bytes.len() + count, byte);
        Ok(())
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn trim(&mut self, value: impl Fn(u8) -> bool, minimum: usize) {
        while self.len() > minimum && self.bytes.last().is_some_and(|b| value(*b)) {
            self.bytes.pop();
        }
    }
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub(super) struct Secondary {
    pub bytes: Buffer,
    pub characters: usize,
}
impl Secondary {
    pub fn write(
        &mut self,
        offset: usize,
        bytes: &[u8],
        modifier: u8,
        placeholder: u8,
    ) -> Result<(), InspectError> {
        self.pad(offset, placeholder)?;
        if !bytes.is_empty() {
            self.bytes.append(bytes)?;
            self.characters += bytes.len();
        } else if let Some(last) = self.bytes.bytes_mut().last_mut() {
            *last = last.wrapping_add(modifier);
        } else {
            self.bytes.push(modifier)?;
        }
        Ok(())
    }
    pub fn pad(&mut self, offset: usize, placeholder: u8) -> Result<(), InspectError> {
        if offset > self.characters {
            self.bytes.fill(offset - self.characters, placeholder)?;
            self.characters = offset;
        }
        Ok(())
    }
}
