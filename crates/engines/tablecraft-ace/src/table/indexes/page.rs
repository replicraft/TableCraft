use super::header::IndexHeader;
use super::profile::KeyWidth;
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
const CHILD_BYTES: usize = size_of::<u32>();
const MAX_KEY_BYTES: usize = size_of::<i64>();
pub(super) const MARKER_BYTES: usize = size_of::<u8>();
const MAX_ENTRY_BYTES: usize = MARKER_BYTES + MAX_KEY_BYTES + ROW_BYTES + CHILD_BYTES;
pub(super) const ASC_VALUE: u8 = 0x7f;
pub(super) const DESC_VALUE: u8 = 0x80;
pub(super) const ASC_NULL: u8 = 0;
pub(super) const DESC_NULL: u8 = 0xff;
pub(super) const INT32_SIGN_BIT: u32 = 1 << (u32::BITS - 1);
const INT64_SIGN_BIT: u64 = 1 << (u64::BITS - 1);
const INT16_SIGN_BIT: u16 = 1 << (u16::BITS - 1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct KeyRef {
    pub key: Option<i64>,
    pub row: u32,
}
impl KeyRef {
    pub fn compare(self, other: Self, descending: bool) -> Ordering {
        let key = self.key.cmp(&other.key);
        (if descending { key.reverse() } else { key }).then(self.row.cmp(&other.row))
    }
}
pub(super) struct Entry {
    pub value: KeyRef,
    pub child: u32,
}
pub(super) struct Page {
    pub leaf: bool,
    pub previous: u32,
    pub next: u32,
    pub tail: u32,
    pub entries: Vec<Entry>,
}

pub(super) fn read<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    number: u32,
    owner: u32,
    descending: bool,
    width: KeyWidth,
) -> Result<Page, InspectError> {
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
    let max_entry_bytes = MARKER_BYTES + width.bytes() + ROW_BYTES + CHILD_BYTES;
    if prefix > max_entry_bytes {
        return Err(InspectError::Malformed);
    }
    let count = page[MASK_OFFSET..ENTRY_START]
        .iter()
        .map(|b| b.count_ones() as usize)
        .sum::<usize>();
    reader
        .budget
        .charge_allocation(count as u64, size_of::<Entry>() as u64)?;
    let mut entries: Vec<Entry> = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut first = [0; MAX_ENTRY_BYTES];
    let mut last = 0;
    for (block, bytes) in page[MASK_OFFSET..ENTRY_START]
        .chunks(bitmap::BLOCK_BYTES)
        .enumerate()
    {
        reader.work(bytes.len())?;
        for bit in bitmap::set_bits(bytes) {
            reader.tick()?;
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
            let mut decoded = [0; MAX_ENTRY_BYTES];
            decoded[..shared].copy_from_slice(&first[..shared]);
            decoded[shared..length].copy_from_slice(bytes);
            let marker = decoded[0];
            let null = marker == if descending { DESC_NULL } else { ASC_NULL };
            if !null && marker != if descending { DESC_VALUE } else { ASC_VALUE } {
                return Err(InspectError::Malformed);
            }
            let key_end = MARKER_BYTES + if null { 0 } else { width.bytes() };
            if length != key_end + ROW_BYTES + if leaf { 0 } else { CHILD_BYTES } {
                return Err(InspectError::Malformed);
            }
            let key = if null {
                None
            } else {
                let mut key_bytes = [0; MAX_KEY_BYTES];
                let bytes = &mut key_bytes[..width.bytes()];
                bytes.copy_from_slice(&decoded[MARKER_BYTES..key_end]);
                if descending {
                    for b in &mut *bytes {
                        *b = !*b;
                    }
                }
                Some(match width {
                    KeyWidth::Byte => i64::from(bytes[0]),
                    KeyWidth::Int16 => i64::from(
                        (u16::from_be_bytes(bytes.try_into().map_err(|_| InspectError::Malformed)?)
                            ^ INT16_SIGN_BIT) as i16,
                    ),
                    KeyWidth::Int32 => i64::from(
                        (u32::from_be_bytes(bytes.try_into().map_err(|_| InspectError::Malformed)?)
                            ^ INT32_SIGN_BIT) as i32,
                    ),
                    KeyWidth::Int64 => {
                        (u64::from_be_bytes(bytes.try_into().map_err(|_| InspectError::Malformed)?)
                            ^ INT64_SIGN_BIT) as i64
                    }
                })
            };
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
            if entries
                .last()
                .is_some_and(|previous| previous.value.compare(value, descending) != Ordering::Less)
            {
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
    if leaf && result.tail != 0 || !leaf && result.tail == 0 {
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
    fn decode(chunks: &[&[u8]], prefix: u16, descending: bool, width: KeyWidth) -> Page {
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
            KeyWidth::Int32,
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
            KeyWidth::Int32,
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
            KeyWidth::Int64,
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
