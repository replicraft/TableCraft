//! Text decoding on the shared bounded native long-value storage.
use super::long_value;
use crate::limits::MAX_LONG_TEXT_BYTES;
use crate::{
    InspectError,
    format::{reader::Reader, text},
};

// Logical input passes for compression classification, UTF-16 conversion and
// UTF-8 emission. The bounded decoder checks cancellation before/after this work.
const DECODE_PASSES: usize = 3;

pub(super) fn read<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    definition: &[u8],
) -> Result<Option<String>, InspectError> {
    long_value::read(reader, definition)?
        .map(|bytes| {
            reader.work(
                bytes
                    .len()
                    .checked_mul(DECODE_PASSES)
                    .ok_or(InspectError::StructureLimit)?,
            )?;
            let decoded = text::decode_long_with_limit(
                &bytes,
                reader.source.long_value_limits().maximum_bytes() as usize,
                reader.budget,
            )?;
            if decoded.len() > MAX_LONG_TEXT_BYTES {
                return Err(InspectError::StructureLimit);
            }
            reader.tick()?;
            Ok(decoded)
        })
        .transpose()
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
        ResourceBudget::new(16 * 1024 * 1024, 1000, 10_000_000)
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
            decode(&definition(0x8000_0000, 2_097_153, 0), &[], &mut budget()).err(),
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
    #[test]
    fn expanded_utf8_obeys_the_decoded_text_ceiling() {
        let payload = [b"\xff\xfe".as_slice(), &vec![0xff; 524_289]].concat();
        let mut def = definition(0x8000_0000, payload.len(), 0);
        def.extend(payload);
        assert_eq!(
            decode(&def, &[], &mut budget()).err(),
            Some(InspectError::StructureLimit)
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
