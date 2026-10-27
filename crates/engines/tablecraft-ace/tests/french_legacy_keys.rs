use sha2::{Digest, Sha256};
use tablecraft_ace::{NativeSortOrder, encode_utf16_text_index_key};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 0, 1_000_000)
}

fn order() -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([12, 4, 0, 0])
}

fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|part| u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
        .collect()
}

fn units(hex: &str) -> Vec<u16> {
    if hex == "-" {
        return Vec::new();
    }

    hex.as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|part| u16::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn independent_french_vectors_cover_secondary_spans_expansions_and_overruns() {
    for (index, line) in include_str!("french_legacy_keys/vectors.tsv")
        .lines()
        .enumerate()
    {
        let fields: Vec<_> = line.split('\t').collect();
        let units = units(fields[0]);
        for (descending, expected) in [(false, fields[1]), (true, fields[2])] {
            let actual =
                encode_utf16_text_index_key(order(), &units, descending, &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(
                actual.as_bytes(),
                bytes(expected),
                "vector {index}, descending {descending}"
            );
        }
    }
}

#[test]
fn every_french_utf16_unit_matches_independent_both_direction_digests() {
    for line in include_str!("french_legacy_keys/bmp.sha256").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        let first = u16::from_str_radix(fields[0], 16).unwrap() as u32 * 256;
        let mut hash = Sha256::new();
        for unit in first..first + 256 {
            for descending in [false, true] {
                let actual = encode_utf16_text_index_key(
                    order(),
                    &[unit as u16],
                    descending,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                hash.update((unit as u16).to_be_bytes());
                hash.update((actual.as_bytes().len() as u16).to_be_bytes());
                hash.update(actual.as_bytes());
            }
        }
        assert_eq!(
            hash.finalize().as_slice(),
            bytes(fields[1]),
            "block {}",
            fields[0]
        );
    }
}
