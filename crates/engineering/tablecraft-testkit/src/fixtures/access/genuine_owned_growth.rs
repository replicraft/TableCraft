//! Explicit independent original, growth and second-edit artifact loading.
use std::{env, fs, path::PathBuf};

pub const OLD: &str = "unchanged sentinel";
pub const GROWN: &str = "aaaaaaaaaaaaaaaaaaa";
pub const SECOND: &str = "bbbbbbbbbbbbbbbbbbb";

pub struct Case {
    pub header: u8,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub second: Vec<u8>,
    pub data: u32,
    pub property_offset: usize,
}

/// Verify every input before returning any case to a target-owning consumer.
/// No engine, codec, creator or model is imported by this std-only owner.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let originals = PathBuf::from(env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let first = PathBuf::from(env::var_os("TABLECRAFT_OWNED_GROWTH_EXPECTED_DIR").unwrap());
    let second = PathBuf::from(env::var_os("TABLECRAFT_OWNED_GROWTH_SECOND_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && first.is_absolute() && second.is_absolute());
    let mut cases = Vec::new();
    for (header, length, original_sha, first_sha, second_sha, data, property_offset) in [
        (
            5_u8,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "2bfdbfc9caddaa5151bbd3006f095e49c4afeba02d0d5ac9b4ffeb84439ce17e",
            "676ed3c5336a1e684673e6c870fc21df467a1252d2fc11907d2b6e3e6ca76cd1",
            110,
            450474,
        ),
        (
            6_u8,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "4cd5c2f9ca8c65e2cb74ec0269712adb8662b0457af6ecce121743dcec1a5775",
            "6be76c53b15d57c616ced2ae0fb73cba419ee0c8b1880e14ebd78d736a565a9a",
            127,
            470685,
        ),
    ] {
        let before = fs::read(originals.join(format!("h{header}.accdb"))).unwrap();
        let after = fs::read(first.join(format!("h{header}-marker-owned-growth.accdb"))).unwrap();
        let second =
            fs::read(second.join(format!("h{header}-marker-forwarded-second.accdb"))).unwrap();
        for (bytes, expected) in [
            (&before, original_sha),
            (&after, first_sha),
            (&second, second_sha),
        ] {
            assert_eq!(bytes.len(), length);
            verify(bytes, expected);
        }
        cases.push(Case {
            header,
            before,
            after,
            second,
            data,
            property_offset,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
