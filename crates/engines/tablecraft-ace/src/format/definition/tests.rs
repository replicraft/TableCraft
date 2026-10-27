use super::*;
use crate::Header;
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn image(length: usize) -> (Vec<u8>, Vec<u8>) {
    // Independent layout facts: first payload 4096 bytes, continuation header 8.
    let pages = 1 + length.saturating_sub(4096).div_ceil(4088);
    let mut file = vec![0; (pages + 1).max(3) * 4096];
    file[..4].copy_from_slice(&[0, 1, 0, 0]);
    file[4..20].copy_from_slice(b"Standard ACE DB\0");
    file[20] = 2;
    file[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    let mut logical: Vec<_> = (0..length).map(|i| i as u8).collect();
    logical[..8].copy_from_slice(&[2, 1, 0, 0, 0, 0, 0, 0]);
    put32(&mut logical, 8, length as u32);
    let first = length.min(4096);
    file[4096..4096 + first].copy_from_slice(&logical[..first]);
    let mut position = first;
    for page in 1..=pages {
        let at = page * 4096;
        file[at..at + 2].copy_from_slice(&[2, 1]);
        put32(
            &mut file,
            at + 4,
            if page < pages { page as u32 + 1 } else { 0 },
        );
        if page > 1 {
            let take = (length - position).min(4088);
            file[at + 8..at + 8 + take].copy_from_slice(&logical[position..position + take]);
            position += take;
        }
    }
    // The first page's chain reference is part of the retained logical header.
    logical[4..8].copy_from_slice(&file[4100..4104]);
    (file, logical)
}
fn decode(
    file: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Definition, InspectError> {
    let header = Header::parse(&file, file.len() as u64)?;
    read(
        &mut Reader {
            source: &mut Cursor::new(file),
            header,
            budget,
            cancelled,
            reads: 0,
        },
        1,
    )
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(4 * 1024 * 1024, 0, 1_000_000)
}

#[test]
fn logical_bytes_cross_headers_and_end_exactly_at_declared_length() {
    for length in [12, 4096, 4097, 8184, 8185, 28079, 261640] {
        let (file, expected) = image(length);
        let actual = decode(file, &mut budget(), &|| false).unwrap();
        assert_eq!(&*actual, expected);
        assert_eq!(actual.joined.is_empty(), length <= 4096);
    }
}
#[test]
fn malformed_sizes_links_cycles_signatures_and_truncations_fail() {
    for change in 0..10 {
        let (mut file, _) = image(9000);
        match change {
            0 => put32(&mut file, 4096 + 8, 0),
            1 => put32(&mut file, 4096 + 8, 261641),
            2 => put32(&mut file, 4096 + 8, 4096),
            3 => put32(&mut file, 4096 + 4, 0),
            4 => file[2 * 4096] = 1,
            5 => put32(&mut file, 2 * 4096 + 4, 2),
            6 => put32(&mut file, 3 * 4096 + 4, 1),
            7 => put32(&mut file, 4096 + 4, 5000),
            8 => put32(&mut file, 2 * 4096 + 4, 0),
            _ => file.truncate(3 * 4096),
        }
        assert!(decode(file, &mut budget(), &|| false).is_err(), "{change}");
    }
}
#[test]
fn chain_allocations_reads_work_and_cancellation_share_caller_allowances() {
    for allowance in [
        ResourceBudget::new(0, 0, 1000),
        ResourceBudget::new(4096, 0, 1000),
        ResourceBudget::new(100000, 0, 0),
    ] {
        assert!(matches!(
            decode(image(9000).0, &mut { allowance }, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
    let checks = Cell::new(0);
    let cancel = || {
        checks.set(checks.get() + 1);
        checks.get() > 5
    };
    assert!(matches!(
        decode(image(9000).0, &mut budget(), &cancel),
        Err(InspectError::Cancelled)
    ));
}
proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5441_424c_5444_4546),
        ..Default::default()
    })]
    #[test]
    fn mutated_chains_are_bounded_and_never_return_partial_bytes(
        edits in prop::collection::vec((4096_usize..4 * 4096, any::<u8>()), 0..16)
    ) {
        let (mut file, _) = image(9000);
        for (at, value) in edits {
            file[at] = value;
        }

        if let Ok(actual) = decode(file, &mut budget(), &|| false) {
            prop_assert_eq!(actual.len(), actual.length);
            prop_assert!(actual.len() <= 261640);
        }
    }
}

mod visits;
