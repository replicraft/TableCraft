use super::*;
use std::cell::Cell;

// Independently assembled with Python struct.pack and hashlib, using literal
// protocol widths/offsets rather than the production layout or encoder.
const GOLDEN: &str = concat!(
    "54434a57494e30310100000002000000f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
    "0807060504030201000102030405060708090a0b0c0d0e0f",
    "090000000000000020202020202020202020202020202020",
    "1817161514131211808182838485868788898a8b8c8d8e8f",
    "090000000000000020202020202020202020202020202020",
    "0300000000000000e300000000000000",
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    "00000000000000000000000000000000",
    "af2878982f97eea0291749c3db1093f5c32695a77ae711928e46d7f845259ab4",
);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 1_000_000, 1_000_000)
}

fn literal() -> Vec<u8> {
    GOLDEN
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn objects() -> FlatObjects {
    FlatObjects {
        target: FlatIdentity {
            volume: 0x0102_0304_0506_0708,
            file_id: std::array::from_fn(|index| index as u8),
        },
        target_parent: FlatIdentity {
            volume: 9,
            file_id: [0x20; 16],
        },
        sidecar: FlatIdentity {
            volume: 0x1112_1314_1516_1718,
            file_id: std::array::from_fn(|index| 0x80 + index as u8),
        },
        sidecar_parent: FlatIdentity {
            volume: 9,
            file_id: [0x20; 16],
        },
    }
}

fn captured() -> FlatBindingData {
    FlatBindingData::capture(
        std::array::from_fn(|index| 0xf0 + index as u8),
        objects(),
        b"abc",
        3,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn resign(bytes: &mut [u8]) {
    let digest = Sha256::digest(&bytes[..192]);
    bytes[192..224].copy_from_slice(&digest);
}

#[test]
fn independent_golden_roundtrips_exact_header_and_intent_extent() {
    let bytes = literal();
    assert_eq!(bytes.len(), 224);
    let data = captured();
    assert_eq!(
        data.encode(3, &mut budget(), &|| false).unwrap().as_slice(),
        bytes
    );
    assert_eq!(decode(&bytes, 3, &mut budget(), &|| false).unwrap(), data);
    assert_eq!(data.journal_offset().unwrap(), 227);
    data.verify_objects(&objects(), &mut budget(), &|| false)
        .unwrap();
    data.verify_intent(b"abc", &mut budget(), &|| false)
        .unwrap();
}

#[test]
fn closed_header_rejects_framing_flags_and_tampering_without_decoder_fallback() {
    for length in [0, 8, 223, 225] {
        let mut bytes = literal();
        bytes.resize(length, 0);
        assert_eq!(
            decode(&bytes, 3, &mut budget(), &|| false),
            Err(BindingError::InvalidLength)
        );
    }
    for (offset, value, expected) in [
        (0, b'X', BindingError::InvalidMagic),
        (8, 2, BindingError::UnsupportedProtocol),
        (12, 1, BindingError::UnsupportedIdentityScheme),
        (176, 1, BindingError::InvalidFlags),
        (191, 1, BindingError::InvalidFlags),
    ] {
        let mut bytes = literal();
        bytes[offset] = value;
        resign(&mut bytes);
        assert_eq!(decode(&bytes, 3, &mut budget(), &|| false), Err(expected));
    }
    // Checksum covers nonce, every physical identity, extent and intent digest.
    for offset in [16, 32, 63, 80, 112, 128, 136, 144, 175, 192, 223] {
        let mut bytes = literal();
        bytes[offset] ^= 1;
        assert_eq!(
            decode(&bytes, 3, &mut budget(), &|| false),
            Err(BindingError::ChecksumMismatch)
        );
    }
}

#[test]
fn exact_unpadded_offset_and_explicit_intent_ceiling_are_enforced() {
    let data = captured();
    assert_eq!(
        data.encode(2, &mut budget(), &|| false),
        Err(BindingError::IntentLimit)
    );
    assert_eq!(
        decode(&literal(), 2, &mut budget(), &|| false),
        Err(BindingError::IntentLimit)
    );
    assert_eq!(
        FlatBindingData::capture([0; 16], objects(), b"abc", 2, &mut budget(), &|| false),
        Err(BindingError::IntentLimit)
    );

    for offset in [0u64, 223, 224, 226, 228, u64::MAX] {
        let mut bytes = literal();
        bytes[136..144].copy_from_slice(&offset.to_le_bytes());
        resign(&mut bytes);
        assert_eq!(
            decode(&bytes, 3, &mut budget(), &|| false),
            Err(BindingError::InvalidOffset)
        );
    }
    let mut overflow = data;
    overflow.intent_length = u64::MAX;
    assert_eq!(overflow.journal_offset(), Err(BindingError::InvalidOffset));
    assert_eq!(
        overflow.encode(u64::MAX, &mut budget(), &|| false),
        Err(BindingError::InvalidOffset)
    );
    let mut bytes = literal();
    bytes[128..136].copy_from_slice(&u64::MAX.to_le_bytes());
    resign(&mut bytes);
    assert_eq!(
        decode(&bytes, u64::MAX, &mut budget(), &|| false),
        Err(BindingError::InvalidOffset)
    );
}

#[test]
fn only_parent_parent_alias_is_admitted_and_all_identity_bits_are_compared() {
    for (first, second) in [(0, 1), (0, 2), (0, 3), (1, 2), (2, 3)] {
        let mut bytes = literal();
        let identity = bytes[32 + first * 24..56 + first * 24].to_vec();
        bytes[32 + second * 24..56 + second * 24].copy_from_slice(&identity);
        resign(&mut bytes);
        assert_eq!(
            decode(&bytes, 3, &mut budget(), &|| false),
            Err(BindingError::AliasedRoles)
        );
    }
    let data = captured();
    let mut aliased = objects();
    aliased.sidecar = aliased.target;
    assert_eq!(
        data.verify_objects(&aliased, &mut budget(), &|| false),
        Err(BindingError::AliasedRoles)
    );
    assert_eq!(
        FlatBindingData::capture([0; 16], aliased, b"abc", 3, &mut budget(), &|| false),
        Err(BindingError::AliasedRoles)
    );

    for bit in 0..128 {
        let mut actual = objects();
        actual.target.file_id[bit / 8] ^= 1 << (bit % 8);
        assert_eq!(
            data.verify_objects(&actual, &mut budget(), &|| false),
            Err(BindingError::ObjectMismatch)
        );
    }
    let mut actual = objects();
    actual.target.volume ^= 1 << 63;
    assert_eq!(
        data.verify_objects(&actual, &mut budget(), &|| false),
        Err(BindingError::ObjectMismatch)
    );
}

#[test]
fn verified_intent_distinguishes_length_and_same_length_digest_changes() {
    let data = captured();
    assert_eq!(
        data.verify_intent(b"ab", &mut budget(), &|| false),
        Err(BindingError::IntentLengthMismatch)
    );
    assert_eq!(
        data.verify_intent(b"abd", &mut budget(), &|| false),
        Err(BindingError::IntentDigestMismatch)
    );
    let empty =
        FlatBindingData::capture([0; 16], objects(), b"", 0, &mut budget(), &|| false).unwrap();
    assert_eq!(empty.journal_offset().unwrap(), 224);
    empty.verify_intent(b"", &mut budget(), &|| false).unwrap();
    assert_eq!(empty.intent_digest, <[u8; 32]>::from(Sha256::digest(b"")));
}

#[test]
fn every_operation_honors_exact_step_limits_without_allocation_or_io_budget() {
    let data = captured();
    let literal = literal();
    for operation in 0..5 {
        let run = |budget: &mut ResourceBudget| -> Result<(), BindingError> {
            match operation {
                0 => {
                    data.encode(3, budget, &|| false)?;
                }
                1 => {
                    decode(&literal, 3, budget, &|| false)?;
                }
                2 => {
                    data.verify_objects(&objects(), budget, &|| false)?;
                }
                3 => {
                    data.verify_intent(b"abc", budget, &|| false)?;
                }
                _ => {
                    FlatBindingData::capture([0; 16], objects(), b"abc", 3, budget, &|| false)?;
                }
            }
            Ok(())
        };
        let mut measured = ResourceBudget::new(0, 0, 10_000);
        run(&mut measured).unwrap();
        let paid = 10_000 - measured.remaining(Resource::Steps);
        let mut exact = ResourceBudget::new(0, 0, paid);
        run(&mut exact).unwrap();
        assert_eq!(exact.remaining(Resource::Steps), 0);
        let mut short = ResourceBudget::new(0, 0, paid - 1);
        assert!(matches!(
            run(&mut short),
            Err(BindingError::Io(IoError::Limit(_)))
        ));
    }
}

#[test]
fn hashing_checks_every_multiblock_cancellation_boundary() {
    // Literal 64KiB boundary is independent of the production chunk constant.
    let intent = vec![0x5a; 2 * 65_536 + 1];
    let calls = Cell::new(0);
    FlatBindingData::capture(
        [0; 16],
        objects(),
        &intent,
        intent.len() as u64,
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert!(calls.get() >= 6);
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let result = FlatBindingData::capture(
            [0; 16],
            objects(),
            &intent,
            intent.len() as u64,
            &mut budget(),
            &|| {
                current.set(current.get() + 1);
                current.get() == stop
            },
        );
        assert_eq!(result, Err(BindingError::Io(IoError::Cancelled)));
    }
    let data = captured();
    for result in [
        data.encode(3, &mut budget(), &|| true).map(|_| ()),
        decode(&literal(), 3, &mut budget(), &|| true).map(|_| ()),
        data.verify_objects(&objects(), &mut budget(), &|| true),
        data.verify_intent(b"abc", &mut budget(), &|| true),
    ] {
        assert_eq!(result, Err(BindingError::Io(IoError::Cancelled)));
    }
}
