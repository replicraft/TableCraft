//! Independent original request wire, including caller-written update order.
use super::fixtures::Case;
use sha2::{Digest, Sha256};
use tablecraft_ace::NativeUpdateIntent;

fn carrier(output: &mut Vec<u8>, text: &str) {
    output.extend_from_slice(&[6, 0, 0, 0]);
    output.extend_from_slice(&(text.len() as u32).to_le_bytes());
    output.extend_from_slice(text.as_bytes());
}

pub(super) fn encode(case: &Case, reverse: bool) -> Vec<u8> {
    let mut originals = vec![(0_u64, "aaaaaaaaaaaaaaaaaaa"), (1, "ccccccccccccccccccc")];
    if reverse {
        originals.reverse();
    }
    let mut cells = Vec::new();
    for (ordinal, replacement) in originals {
        let mut cell = Vec::new();
        cell.extend_from_slice(&ordinal.to_le_bytes());
        carrier(&mut cell, "unchanged sentinel");
        carrier(&mut cell, replacement);
        cells.extend_from_slice(&((cell.len() + 4) as u32).to_le_bytes());
        cells.extend_from_slice(&cell);
    }
    let mut body = Vec::from(*b"Sentinel");
    for number in [(20 + 6 + cells.len()) as u32, 6, 2, 1, 0] {
        body.extend_from_slice(&number.to_le_bytes());
    }
    body.extend_from_slice(b"Marker");
    body.extend_from_slice(&cells);
    let mut output = Vec::from(*b"TCNI0006");
    output.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    output.extend_from_slice(&[6, 0, 0, 0]);
    output.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    output.extend_from_slice(&digest.finalize());
    for number in [8_u32, 1, 2, 0] {
        output.extend_from_slice(&number.to_le_bytes());
    }
    output.extend_from_slice(&body);
    output
}

pub(super) fn decode(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&encode(case, false), &mut super::budget(), &|| false).unwrap()
}
