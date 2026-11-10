//! Literal chained LVAL records for descriptor-walk tests, without allocator authority.
use super::{canonical_creation, data_page};

pub struct Chain {
    pub image: Vec<u8>,
    pub descriptor: [u8; 12],
    pub references: Vec<u32>,
}

/// Append one nonempty payload byte per independent LVAL page. These unclaimed
/// pages deliberately do not qualify a complete writer Source or usage maps.
pub fn source(segments: usize) -> Chain {
    assert!(segments > 0 && segments <= 0x3fff_ffff);
    let mut image = canonical_creation::source(canonical_creation::Case::One).bytes;
    let first = image.len() / 4096;
    assert!(first + segments < 0x0100_0000);
    let references: Vec<u32> = (first..first + segments)
        .map(|page| (page as u32) << 8)
        .collect();
    for index in 0..segments {
        let mut body = references
            .get(index + 1)
            .copied()
            .unwrap_or(0)
            .to_le_bytes()
            .to_vec();
        body.push((index % 251) as u8);
        image.extend(data_page(u32::from_le_bytes(*b"LVAL"), &[(0, body)]));
    }
    let mut descriptor = [0; 12];
    descriptor[..4].copy_from_slice(&(segments as u32).to_le_bytes());
    descriptor[4..8].copy_from_slice(&references[0].to_le_bytes());
    Chain {
        image,
        descriptor,
        references,
    }
}
