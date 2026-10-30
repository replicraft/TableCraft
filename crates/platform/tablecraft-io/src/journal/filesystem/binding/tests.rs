use super::{BindingError, PairObjects, decode};
use crate::{IoError, namespace::ObjectIdentity};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

const GOLDENS: &[(&str, &str)] = &[
    (
        "literal_valid",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad8a0945694b21b2bdb162bc568c1e34e8a54e914aeb489cfa3435e3099d86d7a1",
    ),
    (
        "opaque_zero_nonce",
        "54434a50414952310100000001000000000000000000000000000000000000000700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015adc00dd4727afaa846f94eeee07a1da2884b7db095999d05d78de165e54b8f2556",
    ),
    (
        "unknown_protocol",
        "54434a50414952310200000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ada36dee3a7396ac44552383ea969b2cc9c5f4689d6d2dc1d5bdcc7b03a22f27b3",
    ),
    (
        "unknown_identity_scheme",
        "54434a50414952310100000063000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015adbc2d2985604e30a07fccf391fd177c298ccf303813605513cdc8bd6c034354eb",
    ),
    (
        "target_journal_same_object",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c00000000000000070000000000000029000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015adcfc5d79a86b76c5672582821770a58871435bf0b5bff9dfb9d6a68f20c0e58f0",
    ),
    (
        "binding_intent_same_object",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002b0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad49982f9c99b6e09b902e90c0e380d51aa2e7612e3cdec3c3010da4af6b693f65",
    ),
    (
        "truncated",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad8a0945694b21b2bdb162bc568c1e34e8a54e914aeb489cfa3435e3099d86d7",
    ),
    (
        "trailing",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad8a0945694b21b2bdb162bc568c1e34e8a54e914aeb489cfa3435e3099d86d7a100",
    ),
    (
        "checksum_corruption",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad8b0945694b21b2bdb162bc568c1e34e8a54e914aeb489cfa3435e3099d86d7a1",
    ),
    (
        "byte_identical_wrong_target",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad8a0945694b21b2bdb162bc568c1e34e8a54e914aeb489cfa3435e3099d86d7a1",
    ),
    (
        "wrong_actual_intent_bytes",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad8a0945694b21b2bdb162bc568c1e34e8a54e914aeb489cfa3435e3099d86d7a1",
    ),
    (
        "wrong_actual_intent_length",
        "54434a50414952310100000001000000101112131415161718191a1b1c1d1e1f0700000000000000290000000000000007000000000000002a0000000000000007000000000000002b0000000000000007000000000000002c0000000000000007000000000000002d000000000000000300000000000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad8a0945694b21b2bdb162bc568c1e34e8a54e914aeb489cfa3435e3099d86d7a1",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn bytes(text: &str) -> Vec<u8> {
    let (pairs, remainder) = text.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    let digit = |byte| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("nonhex literal"),
    };
    pairs
        .iter()
        .map(|[high, low]| digit(*high) * 16 + digit(*low))
        .collect()
}

#[test]
fn qualify_pair_binding_wire_literals_before_codec() {
    for (name, text) in GOLDENS {
        let record = bytes(text);
        let length = match *name {
            "truncated" => 183,
            "trailing" => 185,
            _ => 184,
        };
        assert_eq!(record.len(), length);
        if length != 184 {
            continue;
        }
        assert_eq!(&record[..8], b"TCJPAIR1");
        let checksum: [u8; 32] = Sha256::digest(&record[..152]).into();
        assert_eq!(checksum == record[152..], *name != "checksum_corruption");
    }
    let record = bytes(GOLDENS[0].1);
    assert_eq!(&record[8..16], &[1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&record[16..32], &(16_u8..32).collect::<Vec<_>>());
    for position in 0..5 {
        let start = 32 + position * 16;
        assert_eq!(
            u64::from_le_bytes(record[start..start + 8].try_into().unwrap()),
            7
        );
        assert_eq!(
            u64::from_le_bytes(record[start + 8..start + 16].try_into().unwrap()),
            41 + position as u64
        );
    }
    assert_eq!(u64::from_le_bytes(record[112..120].try_into().unwrap()), 3);
    assert_eq!(&record[120..152], &Sha256::digest(b"abc")[..]);
}

#[test]
fn pair_binding_codec_matches_independent_record_and_association_controls() {
    for (name, text) in GOLDENS {
        let record = bytes(text);
        let result = decode(&record, &mut budget(), &|| false);
        match *name {
            "unknown_protocol" => assert!(matches!(result, Err(BindingError::UnsupportedProtocol))),
            "unknown_identity_scheme" => assert!(matches!(
                result,
                Err(BindingError::UnsupportedIdentityScheme)
            )),
            "target_journal_same_object" | "binding_intent_same_object" => {
                assert!(matches!(result, Err(BindingError::AliasedRoles)))
            }
            "truncated" | "trailing" => assert!(matches!(result, Err(BindingError::InvalidLength))),
            "checksum_corruption" => assert!(matches!(result, Err(BindingError::ChecksumMismatch))),
            _ => {
                let data = result.unwrap();
                let encoded = data.encode(&mut budget(), &|| false).unwrap();
                assert_eq!(encoded.as_slice(), record.as_slice());
                let mut actual = PairObjects {
                    target: ObjectIdentity {
                        device: 7,
                        inode: 41,
                    },
                    bundle: ObjectIdentity {
                        device: 7,
                        inode: 42,
                    },
                    binding: ObjectIdentity {
                        device: 7,
                        inode: 43,
                    },
                    intent: ObjectIdentity {
                        device: 7,
                        inode: 44,
                    },
                    journal: ObjectIdentity {
                        device: 7,
                        inode: 45,
                    },
                };
                if *name == "byte_identical_wrong_target" {
                    actual.target.inode = 99;
                    assert!(matches!(
                        data.verify_objects(&actual),
                        Err(BindingError::ObjectMismatch)
                    ));
                } else {
                    data.verify_objects(&actual).unwrap();
                }
                let intent = match *name {
                    "wrong_actual_intent_bytes" => b"abd".as_slice(),
                    "wrong_actual_intent_length" => b"abcd".as_slice(),
                    _ => b"abc".as_slice(),
                };
                let checked = data.verify_intent(intent, &mut budget(), &|| false);
                match *name {
                    "wrong_actual_intent_bytes" => {
                        assert!(matches!(checked, Err(BindingError::IntentDigestMismatch)))
                    }
                    "wrong_actual_intent_length" => {
                        assert!(matches!(checked, Err(BindingError::IntentLengthMismatch)))
                    }
                    _ => checked.unwrap(),
                }
            }
        }
    }
    assert!(matches!(
        decode(&bytes(GOLDENS[0].1), &mut budget(), &|| true),
        Err(BindingError::Io(IoError::Cancelled))
    ));
}
