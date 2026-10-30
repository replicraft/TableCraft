//! Immutable record facts consumed by the actual independent placement replay.
use super::super::placement::DataView;
use std::ops::Range;

pub(in crate::table::mutation) trait RecordImage {
    fn reference(&self) -> u32;
    fn root_word(&self) -> u16;
    fn root_range(&self) -> &Range<usize>;
    fn final_offset(&self) -> usize;
    fn original_length(&self) -> usize;
    fn bytes(&self) -> &[u8];
}

pub(in crate::table::mutation) fn compare<R: RecordImage>(
    records: &[R],
    data: DataView<'_>,
    budget: &mut tablecraft_core::limits::ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), crate::EditError> {
    use crate::{EditError, InspectError};
    use tablecraft_core::limits::Resource;

    crate::header::check(budget, cancelled)?;
    if records.len() != data.events.len() {
        return Err(EditError::Preservation);
    }
    for (record, event) in records.iter().zip(data.events) {
        crate::header::check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        let growing = record.bytes().len() > record.original_length();
        let original_end = record
            .final_offset()
            .checked_add(record.original_length())
            .ok_or(EditError::Preservation)?;
        if record.reference() != event.root
            || record.root_word() != event.word
            || record.root_range() != &event.root_range
            || event.target.len() != record.bytes().len()
            || event.appended.is_some() != growing
            || (!growing && event.target != (record.final_offset()..original_end))
        {
            return Err(EditError::Preservation);
        }
        let actual = data.target(event.target.clone(), budget, cancelled)?;
        super::super::record_proof::equal(record.bytes(), actual, budget, cancelled)?;
    }
    Ok(())
}
