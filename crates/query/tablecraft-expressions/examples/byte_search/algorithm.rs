//! Benchmark-only contenders; no production callback contract is replaced.
use memchr::memmem;

pub const MAX_SHORT_NEEDLE: usize = 32;
pub const BLOCK_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Budget,
    Cancelled,
}

pub struct Meter {
    pub work: u64,
    pub checkpoints: u64,
    pub limit: u64,
    pub cancel_at: Option<u64>,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            work: 0,
            checkpoints: 0,
            limit: u64::MAX,
            cancel_at: None,
        }
    }
}

impl Meter {
    fn charge(&mut self, work: usize) -> Result<(), Failure> {
        self.checkpoints += 1;
        if self.cancel_at == Some(self.checkpoints) {
            return Err(Failure::Cancelled);
        }
        let work = u64::try_from(work).map_err(|_| Failure::Budget)?;
        let total = self.work.checked_add(work).ok_or(Failure::Budget)?;
        if total > self.limit {
            return Err(Failure::Budget);
        }
        self.work = total;
        Ok(())
    }
}

pub type Search = fn(&[u8], &[u8], &mut Meter) -> Result<Option<usize>, Failure>;

/// Existing scalar scheduling, over borrowed bytes at start position one.
pub fn scalar(haystack: &[u8], needle: &[u8], meter: &mut Meter) -> Result<Option<usize>, Failure> {
    meter.charge(1)?;
    if haystack.is_empty() {
        return Ok(None);
    }
    if needle.is_empty() {
        return Ok(Some(0));
    }
    for (offset, window) in haystack.windows(needle.len()).enumerate() {
        if candidate(window, needle, meter)? {
            return Ok(Some(offset));
        }
    }
    Ok(None)
}

fn candidate(window: &[u8], needle: &[u8], meter: &mut Meter) -> Result<bool, Failure> {
    meter.charge(1)?;
    for (left, right) in window.iter().zip(needle) {
        meter.charge(1)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A separate experimental work model: exact scalar first candidate, then each
/// bounded span plus one needle preprocessing allowance before calling memmem.
/// A match inside a span can cost more prepaid units than the old scalar search.
/// Long needles use the original scalar contract and may remain quadratic.
pub fn bounded(
    haystack: &[u8],
    needle: &[u8],
    meter: &mut Meter,
) -> Result<Option<usize>, Failure> {
    if needle.len() > MAX_SHORT_NEEDLE {
        return scalar(haystack, needle, meter);
    }
    meter.charge(1)?;
    if haystack.is_empty() {
        return Ok(None);
    }
    if needle.is_empty() {
        return Ok(Some(0));
    }
    let Some(first) = haystack.get(..needle.len()) else {
        return Ok(None);
    };
    if candidate(first, needle, meter)? {
        return Ok(Some(0));
    }

    let mut offset = 1;
    while haystack.len() - offset >= needle.len() {
        let length = BLOCK_BYTES.min(haystack.len() - offset);
        let span = &haystack[offset..offset + length];
        meter.charge(span.len() + needle.len())?;
        if let Some(found) = memmem::find(span, needle) {
            return Ok(Some(offset + found));
        }
        // Revisit exactly the suffix whose candidate windows cross this span.
        offset += span.len() - needle.len() + 1;
    }
    Ok(None)
}

/// Unbounded reference only. A single entry callback does not meter or cancel
/// the whole-haystack memmem search and is unsuitable for production policy.
pub fn reference(
    haystack: &[u8],
    needle: &[u8],
    meter: &mut Meter,
) -> Result<Option<usize>, Failure> {
    meter.charge(1)?;
    if haystack.is_empty() {
        return Ok(None);
    }
    Ok(memmem::find(haystack, needle))
}

#[cfg(test)]
mod tests;
