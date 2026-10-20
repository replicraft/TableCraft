use super::header::IndexHeader;
use super::key::{KeyProfile, MARKER_BYTES};
#[cfg(test)]
use super::{Int64IndexSelection, key::SingleKey, profile::KeyType};
use crate::{
    InspectError,
    format::{bitmap, reader::Reader},
};
use std::{
    cmp::Ordering,
    io::{Read, Seek},
};

const NODE: u8 = 3;
const LEAF: u8 = 4;
const PAGE_VERSION: u8 = 1;
const MASK_OFFSET: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = 453;
pub(super) const ENTRY_START: usize = MASK_OFFSET + MASK_BYTES;
pub(super) const ROW_BYTES: usize = size_of::<u32>();
pub(super) const CHILD_BYTES: usize = size_of::<u32>();
const MAX_KEY_BYTES: usize = size_of::<i64>();
pub(super) const MAX_ENTRY_BYTES: usize = MARKER_BYTES + MAX_KEY_BYTES + ROW_BYTES + CHILD_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct KeyRef<K = Option<i64>> {
    pub key: K,
    pub row: u32,
}
impl<K: Copy + Eq> KeyRef<K> {
    pub fn compare_with<P: KeyProfile<Key = K>>(self, other: Self, profile: &P) -> Ordering {
        profile
            .compare(self.key, other.key)
            .then(self.row.cmp(&other.row))
    }
}
pub(super) struct Entry<K = Option<i64>> {
    pub value: KeyRef<K>,
    pub child: u32,
}
pub(super) struct Page<K = Option<i64>> {
    pub leaf: bool,
    pub previous: u32,
    pub next: u32,
    pub tail: u32,
    pub entries: Vec<Entry<K>>,
}

#[cfg(test)]
pub(super) fn read<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    number: u32,
    owner: u32,
    descending: bool,
    width: KeyType,
) -> Result<Page, InspectError> {
    read_profile::<_, _, MAX_ENTRY_BYTES>(
        reader,
        number,
        owner,
        &SingleKey {
            column: 0,
            key_type: width,
            descending,
            selection: Int64IndexSelection::All,
        },
    )
}

pub(super) fn read_profile<R: Read + Seek, P: KeyProfile, const N: usize>(
    reader: &mut Reader<'_, R>,
    number: u32,
    owner: u32,
    profile: &P,
) -> Result<Page<P::Key>, InspectError> {
    let page = reader.page(number)?;
    let header = IndexHeader::read(&page)?;
    if !matches!(header.kind, NODE | LEAF)
        || header.version != PAGE_VERSION
        || header.owner.get() != owner
    {
        return Err(InspectError::Malformed);
    }
    let leaf = header.kind == LEAF;
    let prefix = usize::from(header.prefix.get());
    let max_entry_bytes = profile.max_key_bytes() + ROW_BYTES + CHILD_BYTES;
    if max_entry_bytes > N {
        return Err(InspectError::Malformed);
    }
    if prefix > max_entry_bytes {
        return Err(InspectError::Malformed);
    }
    let count = page[MASK_OFFSET..ENTRY_START]
        .iter()
        .map(|b| b.count_ones() as usize)
        .sum::<usize>();
    reader
        .budget
        .charge_allocation(count as u64, size_of::<Entry<P::Key>>() as u64)?;
    let mut entries: Vec<Entry<P::Key>> = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut first = [0; N];
    let mut last = 0;
    for (block, bytes) in page[MASK_OFFSET..ENTRY_START]
        .chunks(bitmap::BLOCK_BYTES)
        .enumerate()
    {
        reader.work(bytes.len())?;
        for bit in bitmap::set_bits(bytes) {
            reader.work(profile.components())?;
            let end = block * bitmap::BLOCK_BYTES * u8::BITS as usize + bit;
            if end <= last || ENTRY_START + end > page.len() {
                return Err(InspectError::Malformed);
            }
            let bytes = &page[ENTRY_START + last..ENTRY_START + end];
            let shared = if entries.is_empty() { 0 } else { prefix };
            let length = shared + bytes.len();
            if length > max_entry_bytes {
                return Err(InspectError::Malformed);
            }
            let mut decoded = [0; N];
            decoded[..shared].copy_from_slice(&first[..shared]);
            decoded[shared..length].copy_from_slice(bytes);
            let trailer = ROW_BYTES + if leaf { 0 } else { CHILD_BYTES };
            let key_length = length.checked_sub(trailer).ok_or(InspectError::Malformed)?;
            let (key, key_end) = profile.decode(&decoded[..key_length])?;
            if length != key_end + ROW_BYTES + if leaf { 0 } else { CHILD_BYTES } {
                return Err(InspectError::Malformed);
            }
            let row = u32::from_be_bytes(
                decoded[key_end..key_end + ROW_BYTES]
                    .try_into()
                    .map_err(|_| InspectError::Malformed)?,
            );
            let child = if leaf {
                0
            } else {
                u32::from_be_bytes(
                    decoded[key_end + ROW_BYTES..length]
                        .try_into()
                        .map_err(|_| InspectError::Malformed)?,
                )
            };
            let value = KeyRef { key, row };
            if entries.last().is_some_and(|previous| {
                previous.value.compare_with(value, profile) != Ordering::Less
            }) {
                return Err(InspectError::Malformed);
            }
            if entries.is_empty() {
                if prefix > length {
                    return Err(InspectError::Malformed);
                }
                first = decoded;
            }
            entries.push(Entry { value, child });
            last = end;
        }
    }
    if (entries.is_empty() && prefix != 0)
        || usize::from(header.free.get()) != page.len() - ENTRY_START - last
    {
        return Err(InspectError::Malformed);
    }
    let result = Page {
        leaf,
        previous: header.previous.get(),
        next: header.next.get(),
        tail: header.tail.get(),
        entries,
    };
    if leaf && result.tail != 0 || !leaf && result.tail == 0 && result.entries.is_empty() {
        return Err(InspectError::Malformed);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Header;
    use std::io::Cursor;
    use tablecraft_core::limits::ResourceBudget;
    fn decode(chunks: &[&[u8]], prefix: u16, descending: bool, width: KeyType) -> Page {
        let mut bytes = vec![0; 3 * 4096];
        bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        bytes[20] = 2;
        bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
        let header = Header::parse(&bytes[..66], bytes.len() as u64).unwrap();
        let page = &mut bytes[2 * 4096..];
        page[..2].copy_from_slice(&[4, 1]);
        page[4..8].copy_from_slice(&1_u32.to_le_bytes());
        page[24..26].copy_from_slice(&prefix.to_le_bytes());
        let mut end = 0;
        for chunk in chunks {
            page[480 + end..480 + end + chunk.len()].copy_from_slice(chunk);
            end += chunk.len();
            page[27 + end / 8] |= 1 << (end % 8);
        }
        page[2..4].copy_from_slice(&((4096 - 480 - end) as u16).to_le_bytes());
        let mut source = Cursor::new(bytes);
        let mut budget = ResourceBudget::new(100_000, 100, 10_000);
        read(
            &mut Reader {
                source: &mut source,
                header,
                budget: &mut budget,
                cancelled: &|| false,
                reads: 0,
            },
            2,
            1,
            descending,
            width,
        )
        .unwrap()
    }
    #[test]
    fn golden_shared_prefix_keeps_duplicate_key_row_ids_distinct() {
        let page = decode(
            &[&[0x7f, 0x80, 0, 0, 1, 0, 0, 2, 0], &[1]],
            8,
            false,
            KeyType::Int32,
        );
        assert_eq!(
            page.entries.iter().map(|e| e.value).collect::<Vec<_>>(),
            vec![
                KeyRef {
                    key: Some(1),
                    row: 512
                },
                KeyRef {
                    key: Some(1),
                    row: 513
                }
            ]
        );
    }
    #[test]
    fn golden_descending_keys_and_null_decode_without_float_coercion() {
        let page = decode(
            &[
                &[0x80, 0x7f, 0xff, 0xff, 0xfe, 0, 0, 2, 0],
                &[0x80, 0x80, 0, 0, 0, 0, 0, 2, 1],
                &[0xff, 0, 0, 2, 2],
            ],
            0,
            true,
            KeyType::Int32,
        );
        assert_eq!(
            page.entries.iter().map(|e| e.value.key).collect::<Vec<_>>(),
            vec![Some(1), Some(-1), None]
        );
    }

    #[test]
    fn golden_int64_descending_prefix_preserves_encoded_first_key_and_row_ids() {
        // 2^53 + 1, descending. The second row shares all but its final byte.
        let page = decode(
            &[
                &[
                    0x80, 0x7f, 0xdf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0, 0, 2, 0,
                ],
                &[1],
            ],
            12,
            true,
            KeyType::Int64,
        );
        assert_eq!(
            page.entries
                .iter()
                .map(|entry| entry.value)
                .collect::<Vec<_>>(),
            vec![
                KeyRef {
                    key: Some(9_007_199_254_740_993),
                    row: 512
                },
                KeyRef {
                    key: Some(9_007_199_254_740_993),
                    row: 513
                }
            ]
        );
    }
}
