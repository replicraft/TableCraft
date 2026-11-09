//! Raw local SELECT/WHERE and spelling fixtures; no production codec participates.
use super::{canonical_native17_creation::packed, put16, put32, query_expression_edits};

const PAGE: usize = 4096;

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn variable_row(attribute: u8, names: &[&str], expression: Option<&str>) -> Vec<u8> {
    let mut row = vec![8, 0, 123, 0, 0, 128, attribute, 0, 0, 0, 0, 0, 0];
    let mut offsets = vec![13];
    row.extend([0, 0, 0, 1]);
    offsets.push(row.len());
    for name in names {
        row.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(row.len());
    }
    assert_eq!(offsets.len(), 4);
    if let Some(expression) = expression {
        let text = expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        row.extend((0x8000_0000 | text.len() as u32).to_le_bytes());
        row.extend([0; 8]);
        row.extend(text);
    }
    row.extend((row.len() as u16).to_le_bytes());
    for offset in offsets.into_iter().rev() {
        row.extend((offset as u16).to_le_bytes());
    }
    row.extend([4, 0]);
    row.push(if expression.is_some() { 0x27 } else { 0x1f });
    row
}

/// Eight native fragments, local MSysObjects alias p, two columns and WHERE.
/// Projection roots4/5 and WHERE root6 share DATA page8. No query runs.
pub fn local_where() -> Vec<u8> {
    let mut bytes = query_expression_edits::two_projections();
    let page = &bytes[8 * PAGE..9 * PAGE];
    let mut rows = Vec::new();
    let mut end = PAGE;
    for slot in 0..6 {
        let start = word(page, 14 + 2 * slot);
        rows.push(page[start..end].to_vec());
        end = start;
    }
    rows.insert(3, variable_row(5, &["MSysObjects", "p"], None));
    rows.insert(6, variable_row(8, &["", ""], Some("[p].[Id] >= 2")));
    bytes[8 * PAGE..9 * PAGE].copy_from_slice(&packed(7, rows));
    put32(&mut bytes, 7 * PAGE + 16, 8);
    bytes
}

/// An unchanged noncanonical integer spelling in a live forwarded first column.
pub fn retained_spelling() -> Vec<u8> {
    query_expression_edits::replace_at(&query_expression_edits::two_projections(), 3, "00042")
}

/// Literal Name1 alias bytes are not SQL fragments and must remain unchanged.
pub fn source_with_unrepresentable_alias() -> Vec<u8> {
    let mut bytes = query_expression_edits::two_projections();
    let root = 8 * PAGE + word(&bytes[8 * PAGE..9 * PAGE], 20);
    // "Answer" becomes "An]wer" without production identifier formatting.
    put16(&mut bytes, root + 17 + 2 * 2, b']' as u16);
    bytes
}
