//! Bounded native MEMO decoding. Stored markup/code remains inert text.
use crate::{
    InspectError,
    format::{
        bytes::u32_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, reference_page, reference_slot},
        text,
    },
    profile::{MAX_LONG_TEXT_BYTES, MAX_LONG_VALUE_CHUNKS},
};
use std::io::{Read, Seek};

const DEFINITION_BYTES: usize = 3 * size_of::<u32>();
const LENGTH_TYPE_MASK: u32 = 0xc000_0000;
const INLINE: u32 = 0x8000_0000;
const SINGLE_ROW: u32 = 0x4000_0000;
const CHAINED: u32 = 0;
const REFERENCE_OFFSET: usize = size_of::<u32>();
const CHAIN_REFERENCE_BYTES: usize = size_of::<u32>();
const LVAL_SIGNATURE: &[u8] = b"LVAL";

/// An empty native MEMO definition denotes NULL; a zero-length inline payload
/// denotes empty text. Neither case runs/interprets contents.
pub(super) fn read<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    definition: &[u8],
) -> Result<Option<String>, InspectError> {
    reader.tick()?;
    if definition.is_empty() {
        return Ok(None);
    }
    if definition.len() < DEFINITION_BYTES {
        return Err(InspectError::Truncated);
    }
    let tagged = u32_at(definition, 0)?;
    let length = (tagged & !LENGTH_TYPE_MASK) as usize;
    if length > MAX_LONG_TEXT_BYTES {
        return Err(InspectError::StructureLimit);
    }
    reader.budget.charge_allocation(length as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    match tagged & LENGTH_TYPE_MASK {
        INLINE => {
            if definition.len() != DEFINITION_BYTES + length {
                return Err(InspectError::Malformed);
            }
            bytes.extend_from_slice(&definition[DEFINITION_BYTES..]);
        }
        kind @ (SINGLE_ROW | CHAINED) => {
            if definition.len() != DEFINITION_BYTES || length == 0 {
                return Err(InspectError::Malformed);
            }
            let mut reference = u32_at(definition, REFERENCE_OFFSET)?;
            let mut visited = [0_u32; MAX_LONG_VALUE_CHUNKS];
            let mut complete = false;
            for hop in 0..MAX_LONG_VALUE_CHUNKS {
                reader.tick()?;
                if reference == 0 || visited[..hop].contains(&reference) {
                    return Err(InspectError::Malformed);
                }
                visited[hop] = reference;
                let page = reader.page(reference_page(reference))?;
                if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                    || page.get(OWNER_OFFSET..OWNER_OFFSET + LVAL_SIGNATURE.len())
                        != Some(LVAL_SIGNATURE)
                {
                    return Err(InspectError::Malformed);
                }
                let (flags, start, end) = row_range(&page, usize::from(reference_slot(reference)))?;
                if flags & (DELETED_MASK | RELOCATED_MASK) != 0 {
                    return Err(InspectError::Malformed);
                }
                let row = &page[start..end];
                let payload = if kind == CHAINED {
                    reference = u32_at(row, 0)?;
                    row.get(CHAIN_REFERENCE_BYTES..)
                        .ok_or(InspectError::Truncated)?
                } else {
                    reference = 0;
                    row
                };
                let remaining = length - bytes.len();
                if payload.is_empty() || payload.len() > remaining {
                    return Err(InspectError::Malformed);
                }
                bytes.extend_from_slice(payload);
                if bytes.len() == length {
                    if reference != 0 {
                        return Err(InspectError::Malformed);
                    }
                    complete = true;
                    break;
                }
                if kind == SINGLE_ROW {
                    return Err(InspectError::Truncated);
                }
            }
            if !complete {
                return Err(InspectError::StructureLimit);
            }
        }
        _ => return Err(InspectError::Unsupported),
    }
    reader.tick()?;
    let decoded = text::decode_long(&bytes, reader.budget)?;
    if decoded.len() > MAX_LONG_TEXT_BYTES {
        return Err(InspectError::StructureLimit);
    }
    Ok(Some(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Header;
    use proptest::prelude::*;
    use std::io::Cursor;
    use tablecraft_core::limits::ResourceBudget;
    const PAGE_BYTES: usize = 4096;
    fn page(payload: &[u8]) -> Vec<u8> {
        let mut page = vec![0; PAGE_BYTES];
        page[..2].copy_from_slice(&[1, 1]);
        page[4..8].copy_from_slice(b"LVAL");
        page[12..14].copy_from_slice(&1_u16.to_le_bytes());
        let start = PAGE_BYTES - payload.len();
        page[14..16].copy_from_slice(&(start as u16).to_le_bytes());
        page[start..].copy_from_slice(payload);
        page
    }
    fn definition(kind: u32, length: usize, reference: u32) -> Vec<u8> {
        [
            (kind | length as u32).to_le_bytes(),
            reference.to_le_bytes(),
            0_u32.to_le_bytes(),
        ]
        .concat()
    }
    fn decode(
        def: &[u8],
        pages: &[(usize, Vec<u8>)],
        budget: &mut ResourceBudget,
    ) -> Result<Option<String>, InspectError> {
        let mut bytes = vec![0; 6 * PAGE_BYTES];
        bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        bytes[20] = 2;
        bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
        for (number, page) in pages {
            bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(page);
        }
        let header = Header::parse(&bytes[..66], bytes.len() as u64).unwrap();
        let mut source = Cursor::new(bytes);
        read(
            &mut Reader {
                source: &mut source,
                header,
                budget,
                cancelled: &|| false,
                reads: 0,
            },
            def,
        )
    }
    fn budget() -> ResourceBudget {
        ResourceBudget::new(16 * 1024 * 1024, 1000, 1000)
    }
    #[test]
    fn inline_single_and_chained_values_preserve_text_and_null_empty_distinctions() {
        let text = b"\xff\xfeHello";
        let mut inline = definition(0x8000_0000, text.len(), 0);
        inline.extend_from_slice(text);
        assert_eq!(
            decode(&inline, &[], &mut budget()).unwrap(),
            Some("Hello".into())
        );
        assert_eq!(
            decode(
                &definition(0x4000_0000, text.len(), 4 << 8),
                &[(4, page(text))],
                &mut budget()
            )
            .unwrap(),
            Some("Hello".into())
        );
        let first = [(5_u32 << 8).to_le_bytes().as_slice(), &text[..1]].concat();
        let last = [0_u32.to_le_bytes().as_slice(), &text[1..]].concat();
        assert_eq!(
            decode(
                &definition(0, text.len(), 4 << 8),
                &[(4, page(&first)), (5, page(&last))],
                &mut budget()
            )
            .unwrap(),
            Some("Hello".into())
        );
        assert_eq!(decode(&[], &[], &mut budget()).unwrap(), None);
        assert_eq!(
            decode(&definition(0x8000_0000, 0, 0), &[], &mut budget()).unwrap(),
            Some(String::new())
        );
    }
    #[test]
    fn malformed_lengths_cycles_foreign_pages_invalid_text_and_limits_fail() {
        let cycle = [(4_u32 << 8).to_le_bytes().as_slice(), b"ab"].concat();
        assert!(
            decode(
                &definition(0, 8, 4 << 8),
                &[(4, page(&cycle))],
                &mut budget()
            )
            .is_err()
        );
        let truncated = [0_u32.to_le_bytes().as_slice(), b"ab"].concat();
        assert!(
            decode(
                &definition(0, 8, 4 << 8),
                &[(4, page(&truncated))],
                &mut budget()
            )
            .is_err()
        );
        let mut foreign = page(b"ab");
        foreign[4] = 0;
        assert!(
            decode(
                &definition(0x4000_0000, 2, 4 << 8),
                &[(4, foreign)],
                &mut budget()
            )
            .is_err()
        );
        assert!(
            decode(
                &definition(0x4000_0000, 8, 4 << 8),
                &[(4, page(b"ab"))],
                &mut budget()
            )
            .is_err()
        );
        assert_eq!(
            decode(
                &definition(0x8000_0000, MAX_LONG_TEXT_BYTES + 1, 0),
                &[],
                &mut budget()
            )
            .err(),
            Some(InspectError::StructureLimit)
        );
        let mut invalid = definition(0x8000_0000, 2, 0);
        invalid.extend_from_slice(&[0, 0xd8]);
        assert!(decode(&invalid, &[], &mut budget()).is_err());
        assert!(decode(&definition(0xc000_0000, 2, 0), &[], &mut budget()).is_err());
        assert!(
            decode(
                &definition(0x4000_0000, 2, 4 << 8),
                &[(4, page(b"ab"))],
                &mut ResourceBudget::new(1, 1000, 1000)
            )
            .is_err()
        );
    }
    proptest! {
        #![proptest_config(proptest::test_runner::Config{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4c4f4e4754455854),..Default::default()})]
        #[test]
        fn arbitrary_definitions_are_bounded_and_never_panic(bytes in prop::collection::vec(any::<u8>(),0..128)) {
            let _=decode(&bytes,&[],&mut budget());
        }
    }
}
