//! Literal complete records frozen before the resizable transaction writer.
use sha2::{Digest, Sha256};

pub(in crate::journal) struct Golden {
    pub name: &'static str,
    pub before: &'static [u8],
    pub after: &'static [u8],
    pub pending: &'static str,
    pub committed: &'static str,
}

pub(in crate::journal) const CASES: &[Golden] = &[
    Golden {
        name: "grow",
        before: b"\x6f\x6c\x64",
        after: b"\x72\x65\x70\x6c\x61\x63\x65\x6d\x65\x6e\x74",
        pending: "5443494d4730303203000000000000000b000000000000003eb40b2d78ebc7592093bf66c34d38970d328493d4ad4c95afa065fd2ea6d70a6f6c647265706c6163656d656e74",
        committed: "5443494d4730303203000000000000000b000000000000003eb40b2d78ebc7592093bf66c34d38970d328493d4ad4c95afa065fd2ea6d70a6f6c647265706c6163656d656e74434f4d4d495430323eb40b2d78ebc7592093bf66c34d38970d328493d4ad4c95afa065fd2ea6d70a",
    },
    Golden {
        name: "shrink",
        before: b"\x62\x65\x66\x6f\x72\x65\x2d\x74\x61\x69\x6c",
        after: b"\x6e\x65\x77",
        pending: "5443494d473030320b000000000000000300000000000000ad6d0a7e79621d8350ad454b92c2c8b8172e767018d08a4b6fd13810c656c5576265666f72652d7461696c6e6577",
        committed: "5443494d473030320b000000000000000300000000000000ad6d0a7e79621d8350ad454b92c2c8b8172e767018d08a4b6fd13810c656c5576265666f72652d7461696c6e6577434f4d4d49543032ad6d0a7e79621d8350ad454b92c2c8b8172e767018d08a4b6fd13810c656c557",
    },
    Golden {
        name: "equal",
        before: b"\x6f\x72\x69\x67\x69\x6e\x61\x6c",
        after: b"\x6d\x6f\x64\x69\x66\x69\x65\x64",
        pending: "5443494d4730303208000000000000000800000000000000560fb131bfbb335580ba36fd1a4b100083e3394a5a2988bc2d8385487c98ff966f726967696e616c6d6f646966696564",
        committed: "5443494d4730303208000000000000000800000000000000560fb131bfbb335580ba36fd1a4b100083e3394a5a2988bc2d8385487c98ff966f726967696e616c6d6f646966696564434f4d4d49543032560fb131bfbb335580ba36fd1a4b100083e3394a5a2988bc2d8385487c98ff96",
    },
    Golden {
        name: "empty_before",
        before: b"",
        after: b"\x61\x66\x74\x65\x72",
        pending: "5443494d47303032000000000000000005000000000000004649f5ad400552de42153f9f7495c939cbaf6aac335d082211b35b89c10882546166746572",
        committed: "5443494d47303032000000000000000005000000000000004649f5ad400552de42153f9f7495c939cbaf6aac335d082211b35b89c10882546166746572434f4d4d495430324649f5ad400552de42153f9f7495c939cbaf6aac335d082211b35b89c1088254",
    },
    Golden {
        name: "empty_after",
        before: b"\x62\x65\x66\x6f\x72\x65",
        after: b"",
        pending: "5443494d47303032060000000000000000000000000000008596f43dd8578e1ecb85cf2a8f24d913c81c1fcb32979ae36f34785ec198304f6265666f7265",
        committed: "5443494d47303032060000000000000000000000000000008596f43dd8578e1ecb85cf2a8f24d913c81c1fcb32979ae36f34785ec198304f6265666f7265434f4d4d495430328596f43dd8578e1ecb85cf2a8f24d913c81c1fcb32979ae36f34785ec198304f",
    },
    Golden {
        name: "empty_both",
        before: b"",
        after: b"",
        pending: "5443494d47303032000000000000000000000000000000003a3f1fe9b17169fa638d59dcb312f3662ad312e2f5d3bf2ff6d4944783ea0499",
        committed: "5443494d47303032000000000000000000000000000000003a3f1fe9b17169fa638d59dcb312f3662ad312e2f5d3bf2ff6d4944783ea0499434f4d4d495430323a3f1fe9b17169fa638d59dcb312f3662ad312e2f5d3bf2ff6d4944783ea0499",
    },
    Golden {
        name: "zero_payload",
        before: b"\x61\x00\x62",
        after: b"\x00\x63",
        pending: "5443494d47303032030000000000000002000000000000001946a2d2421a771c31b27612911889e0111ba18fe5968781930abbe02106b2086100620063",
        committed: "5443494d47303032030000000000000002000000000000001946a2d2421a771c31b27612911889e0111ba18fe5968781930abbe02106b2086100620063434f4d4d495430321946a2d2421a771c31b27612911889e0111ba18fe5968781930abbe02106b208",
    },
    Golden {
        name: "split_ab_c",
        before: b"\x61\x62",
        after: b"\x63",
        pending: "5443494d4730303202000000000000000100000000000000891747661c02c96112568c7f247da305078d4b118f28b27f87f61fe1cf660f06616263",
        committed: "5443494d4730303202000000000000000100000000000000891747661c02c96112568c7f247da305078d4b118f28b27f87f61fe1cf660f06616263434f4d4d49543032891747661c02c96112568c7f247da305078d4b118f28b27f87f61fe1cf660f06",
    },
    Golden {
        name: "split_a_bc",
        before: b"\x61",
        after: b"\x62\x63",
        pending: "5443494d47303032010000000000000002000000000000007dcfe46df2f4e214af13b573d48194218908e47cbf7df23ba463007f917c6b73616263",
        committed: "5443494d47303032010000000000000002000000000000007dcfe46df2f4e214af13b573d48194218908e47cbf7df23ba463007f917c6b73616263434f4d4d495430327dcfe46df2f4e214af13b573d48194218908e47cbf7df23ba463007f917c6b73",
    },
];

pub(in crate::journal) fn bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2));
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn qualify_resizable_record_goldens_before_transactions() {
    for case in CASES {
        let pending = bytes(case.pending);
        let committed = bytes(case.committed);
        assert_eq!(&pending[..8], b"TCIMG002", "{}", case.name);
        assert_eq!(
            u64::from_le_bytes(pending[8..16].try_into().unwrap()),
            case.before.len() as u64
        );
        assert_eq!(
            u64::from_le_bytes(pending[16..24].try_into().unwrap()),
            case.after.len() as u64
        );
        assert_eq!(pending.len(), 56 + case.before.len() + case.after.len());
        assert_eq!(&pending[56..56 + case.before.len()], case.before);
        assert_eq!(&pending[56 + case.before.len()..], case.after);

        let mut hash = Sha256::new();
        hash.update(&pending[..24]);
        hash.update(case.before);
        hash.update(case.after);
        let actual: [u8; 32] = hash.finalize().into();
        assert_eq!(&pending[24..56], actual.as_slice());
        assert_eq!(&committed[..pending.len()], &pending);
        assert_eq!(&committed[pending.len()..pending.len() + 8], b"COMMIT02");
        assert_eq!(&committed[pending.len() + 8..], &pending[24..56]);
    }

    let first = bytes(CASES[7].pending);
    let second = bytes(CASES[8].pending);
    assert_eq!(&first[56..], &second[56..]);
    assert_ne!(&first[24..56], &second[24..56]);
}
