//! Single Short/Long Text physical keys. Native key equivalence is not SQL equality.
use super::entry_digest as digest;
mod equality;
mod selection;
use super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyProfile, KeyRow, KeyWork, MARKER_BYTES},
    page::{CHILD_BYTES, ROW_BYTES},
};
use crate::{
    InspectError, NativeColumn, NativeSortOrder, encode_text_index_key,
    format::data_type::{LONG_TEXT, MAX_SHORT_TEXT_UNITS},
    format::layout::INDEX_KEY_BYTES,
    header::check,
};
pub use equality::GeneralEqualityPrefix;
use selection::EncodedSelection;
pub use selection::TextIndexSelection;
use std::cmp::Ordering;
use tablecraft_core::{limits::Resource, values::Value};

// Entry-level truncation includes the value marker and uses the native
// physical-key ceiling. The discarded suffix is replaced by a two-byte digest.
pub(super) const MAX_KEY_BYTES: usize = INDEX_KEY_BYTES;
pub(super) const MAX_ENTRY_BYTES: usize = MAX_KEY_BYTES + ROW_BYTES + CHILD_BYTES;
const UNIT_BYTES: usize = size_of::<u16>();
const PRIMARY_END: u8 = 1;
const PAYLOAD_END: u8 = 0;

#[derive(Eq, PartialEq)]
pub(super) struct TextKey(Vec<u8>);

#[cfg(feature = "ace16-write-preview")]
impl TextKey {
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

pub(super) struct TextProfile {
    column: usize,
    descending: bool,
    order: NativeSortOrder,
    // None is Memo: records retain their full bounded value, while physical
    // indexing uses only the encoder's native prefix.
    maximum_units: Option<usize>,
    selection: EncodedSelection,
}
impl TextProfile {
    pub fn new(
        column: usize,
        metadata: &NativeColumn,
        descending: bool,
    ) -> Result<Self, InspectError> {
        let order = metadata
            .text_sort_order()
            .ok_or(InspectError::Unsupported)?;
        let long_text = metadata.native_type() == LONG_TEXT;
        let supported =
            order.general_family().is_some() || (!long_text && order.is_french_legacy());
        if !supported
            || (!long_text
                && (!metadata.storage_length().is_multiple_of(UNIT_BYTES)
                    || metadata.storage_length() / UNIT_BYTES > MAX_SHORT_TEXT_UNITS))
        {
            return Err(InspectError::Unsupported);
        }
        Ok(Self {
            column,
            descending,
            order,
            maximum_units: (!long_text).then_some(metadata.storage_length() / UNIT_BYTES),
            selection: EncodedSelection::All,
        })
    }
    pub fn select(
        &mut self,
        selection: TextIndexSelection<'_>,
        work: &mut KeyWork<'_>,
    ) -> Result<(), InspectError> {
        self.selection = EncodedSelection::new(selection, self, work)?;
        Ok(())
    }
    fn encode(
        &self,
        text: &str,
        maximum_units: Option<usize>,
        work: &mut KeyWork<'_>,
    ) -> Result<TextKey, InspectError> {
        let mut bytes = self.encode_component(text, maximum_units, work)?;
        digest::truncate(&mut bytes, work)?;
        check(work.budget, work.cancelled)?;
        Ok(TextKey(bytes))
    }
    /// Encode a complete component before the whole-entry digest is applied.
    fn encode_component(
        &self,
        text: &str,
        maximum_units: Option<usize>,
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        if let Some(maximum_units) = maximum_units {
            for (position, _) in text.encode_utf16().enumerate() {
                check(work.budget, work.cancelled)?;
                if position >= maximum_units {
                    return Err(InspectError::Malformed);
                }
            }
        }
        let payload = encode_text_index_key(
            self.order,
            text,
            self.descending,
            work.budget,
            work.cancelled,
        )?;
        copy_bytes(&[self.value_marker()], payload.as_bytes(), work)
    }
    pub(super) fn row_component(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        match row.get(self.column).ok_or(InspectError::Malformed)? {
            Value::Null => copy_bytes(&[self.null_marker()], &[], work),
            Value::Text(text) => self.encode_component(text, self.maximum_units, work),
            _ => Err(InspectError::Malformed),
        }
    }
    fn null_marker(&self) -> u8 {
        if self.descending { DESC_NULL } else { ASC_NULL }
    }
    fn value_marker(&self) -> u8 {
        if self.descending {
            DESC_VALUE
        } else {
            ASC_VALUE
        }
    }
}

impl KeyProfile for TextProfile {
    type Key = TextKey;
    fn components(&self) -> usize {
        1
    }
    fn max_key_bytes(&self) -> usize {
        MAX_KEY_BYTES
    }
    fn decode(
        &self,
        bytes: &[u8],
        work: &mut KeyWork<'_>,
    ) -> Result<(TextKey, usize), InspectError> {
        check(work.budget, work.cancelled)?;
        work.budget.charge(Resource::Steps, bytes.len() as u64)?;
        let marker = *bytes.first().ok_or(InspectError::Malformed)?;
        if marker == self.null_marker() {
            if bytes.len() != MARKER_BYTES {
                return Err(InspectError::Malformed);
            }
        } else if marker == self.value_marker() {
            let payload = &bytes[MARKER_BYTES..];
            let (end, separator) = if self.descending {
                (&[!PAYLOAD_END, PAYLOAD_END][..], !PRIMARY_END)
            } else {
                (&[PAYLOAD_END][..], PRIMARY_END)
            };
            if payload.len() <= end.len()
                || (bytes.len() != MAX_KEY_BYTES
                    && (!payload.ends_with(end) || !payload.contains(&separator)))
            {
                return Err(InspectError::Malformed);
            }
        } else {
            return Err(InspectError::Malformed);
        }
        if bytes.len() > MAX_KEY_BYTES {
            return Err(InspectError::Malformed);
        }
        let key = copy_key(&[], bytes, work)?;
        Ok((key, bytes.len()))
    }
    fn compare(&self, left: &TextKey, right: &TextKey) -> Ordering {
        left.0.cmp(&right.0)
    }
    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<TextKey, InspectError> {
        check(work.budget, work.cancelled)?;
        match row.get(self.column).ok_or(InspectError::Malformed)? {
            Value::Null => copy_key(&[self.null_marker()], &[], work),
            Value::Text(text) => self.encode(text, self.maximum_units, work),
            _ => Err(InspectError::Malformed),
        }
    }
    fn complete(&self) -> bool {
        matches!(self.selection, EncodedSelection::All)
    }
    fn contains(&self, key: &TextKey) -> bool {
        self.selection.contains(&key.0, self.null_marker())
    }
    fn may_match(
        &self,
        lower: Option<&super::page::KeyRef<TextKey>>,
        upper: Option<&super::page::KeyRef<TextKey>>,
    ) -> bool {
        self.selection.may_match(
            lower.map(|entry| entry.key.0.as_slice()),
            upper.map(|entry| entry.key.0.as_slice()),
            self.null_marker(),
        )
    }
    fn unique_key_is_exact(&self, key: &TextKey) -> bool {
        // Memo prefix equality and overflow-digest equality cannot establish
        // duplicate full values; retain an explicit unique-check gap.
        self.maximum_units.is_some() && key.0.len() < MAX_KEY_BYTES
    }
    fn constraint_nulls(&self, key: &TextKey) -> Result<(bool, bool), InspectError> {
        let null = key.0 == [self.null_marker()];
        Ok((null, null))
    }
}

fn copy_key(
    prefix: &[u8],
    payload: &[u8],
    work: &mut KeyWork<'_>,
) -> Result<TextKey, InspectError> {
    let mut bytes = copy_bytes(prefix, payload, work)?;
    digest::truncate(&mut bytes, work)?;
    check(work.budget, work.cancelled)?;
    Ok(TextKey(bytes))
}

fn copy_bytes(
    prefix: &[u8],
    payload: &[u8],
    work: &mut KeyWork<'_>,
) -> Result<Vec<u8>, InspectError> {
    check(work.budget, work.cancelled)?;
    let length = prefix.len() + payload.len();
    work.budget
        .charge_allocation(length as u64, size_of::<u8>() as u64)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(prefix);
    bytes.extend_from_slice(payload);
    check(work.budget, work.cancelled)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selection_tests;
