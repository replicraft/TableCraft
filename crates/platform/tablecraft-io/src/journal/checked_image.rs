//! Select one closed image framing from its exact magic, without decoder retries.
use super::{CheckedV1, CheckedV2, DurableStorage, JournalError, inspect_v1, inspect_v2, storage};
use tablecraft_core::limits::ResourceBudget;

const MAGIC_BYTES: usize = 8;
const FIXED_MAGIC: &[u8; MAGIC_BYTES] = b"TCIMG001";
const RESIZABLE_MAGIC: &[u8; MAGIC_BYTES] = b"TCIMG002";

/// One fully inspected journal, retaining its original exclusive storage borrow.
/// Framing integrity supplies neither native admission nor target identity.
/// The same journal remains borrowed regardless of the selected framing:
///
/// ```compile_fail
/// use tablecraft_core::limits::ResourceBudget;
/// use tablecraft_io::journal::{DurableStorage, inspect_image};
/// fn replace<J: DurableStorage>(journal: &mut J, replacement: J) {
///     let mut budget = ResourceBudget::new(100_000, 0, 100_000);
///     let checked = inspect_image(journal, &mut budget, &|| false).unwrap();
///     *journal = replacement;
///     drop(checked);
/// }
/// ```
pub enum CheckedImage<'a, J: DurableStorage> {
    /// Equal-length V1 images with complete integrity and marker checks.
    Fixed(CheckedV1<'a, J>),
    /// Independently length-bound V2 images with integrity and marker checks.
    Resizable(CheckedV2<'a, J>),
}

/// Read the exact framing discriminator, then invoke only that closed inspector.
/// Unsupported/corrupt bytes refuse without trying another decoder or format.
pub fn inspect_image<'a, J: DurableStorage>(
    journal: &'a mut J,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedImage<'a, J>, JournalError> {
    let _span = tracing::debug_span!("io.journal.inspect_image").entered();
    storage::check(budget, cancelled)?;
    if journal.storage_len()? < MAGIC_BYTES as u64 {
        return Err(JournalError::InvalidJournal);
    }

    let mut magic = [0; MAGIC_BYTES];
    storage::read_at(journal, 0, &mut magic, budget, cancelled)?;

    match &magic {
        FIXED_MAGIC => inspect_v1(journal, budget, cancelled).map(CheckedImage::Fixed),
        RESIZABLE_MAGIC => inspect_v2(journal, budget, cancelled).map(CheckedImage::Resizable),
        _ => Err(JournalError::InvalidJournal),
    }
}

#[cfg(test)]
mod tests;
