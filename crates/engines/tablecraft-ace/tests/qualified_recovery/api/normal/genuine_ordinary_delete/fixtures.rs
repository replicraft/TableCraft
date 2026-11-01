//! Original source pins and independently frozen two-location DELETE laws.
use super::budget;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tablecraft_ace::{NativeDeleteIntent, RowDelete};

pub(super) struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub raw: Vec<u8>,
    pub definition: usize,
    pub data: usize,
}

pub(super) fn cases() -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_ORDINARY_DELETE_EXPECTED_DIR").unwrap());
    assert!(directory.is_absolute());
    let pins = [
        (
            5,
            107,
            110,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "187714c3125969f8b47a50b1e83d993f3512fe21911bb0043e650b7dcd2754da",
        ),
        (
            6,
            125,
            127,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "de8da4c54fb46f384102900212376ca1c91768bdf8e6b2c153e4410e844ab03e",
        ),
    ];
    // Both complete sources and expected images are checked before any planner.
    pins.into_iter()
        .map(|(header, definition, data, pin, after_pin)| {
            let before =
                std::fs::read(directory.join(format!("h{header}-ordinary-delete-before.accdb")))
                    .unwrap();
            let after =
                std::fs::read(directory.join(format!("h{header}-ordinary-delete-deleted.accdb")))
                    .unwrap();
            let actual = Sha256::digest(&before)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(actual, pin);
            let actual_after = Sha256::digest(&after)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(actual_after, after_pin);
            assert_eq!(
                &before[definition * 4096 + 16..definition * 4096 + 20],
                &[1, 0, 0, 0]
            );
            assert_eq!(&before[data * 4096 + 14..data * 4096 + 16], &[0xbe, 0x0f]);
            let raw = before[data * 4096 + 4030..data * 4096 + 4096].to_vec();
            assert_eq!(raw.len(), 66);
            let mut literal = before.clone();
            literal[definition * 4096 + 16..definition * 4096 + 20].fill(0);
            literal[data * 4096 + 14..data * 4096 + 16].copy_from_slice(&[0xbe, 0xcf]);
            assert_eq!(after, literal);
            Case {
                before,
                after,
                raw,
                definition,
                data,
            }
        })
        .collect()
}

pub(super) fn requests(case: &Case) -> Vec<RowDelete> {
    vec![RowDelete::new(0, case.raw.clone())]
}

pub(super) fn wire(case: &Case) -> Vec<u8> {
    let mut bytes = Vec::from(*b"TCND0001");
    bytes.extend_from_slice(&154_u32.to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((case.before.len() as u64).to_le_bytes());
    hash.update(&case.before);
    bytes.extend_from_slice(&hash.finalize());
    for value in [8_u32, 1, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(b"Sentinel");
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&66_u32.to_le_bytes());
    bytes.extend_from_slice(&case.raw);
    assert_eq!(bytes.len(), 154);
    bytes
}

pub(super) fn intent(case: &Case) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}
