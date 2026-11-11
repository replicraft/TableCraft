//! Native-sized direct companions plus one materialized ordinary LONG output.
use super::super::exact_values;
use super::{Column, EXACT, NUMBER, append};

/// Exact rows retain source NULLs; the computed LONG is an ordinary nullable value.
pub struct Row<'a> {
    /// Uncompressed Text with capacity seven native UTF-16 units.
    pub label: Option<&'a str>,
    /// Signed coefficient at the explicitly fixed scale four.
    pub amount: Option<i128>,
    /// Ordinary LONG value; no generation or stored expression is implied.
    pub score: Option<i32>,
}

/// Independent Text7 / Decimal28,4 / LONG image; no keys, defaults or expressions.
pub fn after(before: &[u8], table: &str, values: &[Row<'_>]) -> Vec<u8> {
    let [label, amount] = EXACT;
    let [score] = NUMBER;
    let columns: [Column; 3] = [label, amount, score];
    let records = values
        .iter()
        .map(|value| {
            let mut row = vec![3, 0];
            row.extend(value.score.unwrap_or(0).to_le_bytes());
            if let Some(label) = value.label {
                assert!(!label.starts_with('\u{feff}'));
                assert!(label.encode_utf16().count() <= 7);
                row.extend(label.encode_utf16().flat_map(u16::to_le_bytes));
            }
            let decimal_start = row.len();
            if let Some(amount) = value.amount {
                assert!(amount.unsigned_abs() < 10_u128.pow(28));
                row.extend(exact_values::decimal_bytes(amount));
            }
            let end = row.len();
            for offset in [end, decimal_start, 6, 2] {
                row.extend((offset as u16).to_le_bytes());
            }
            row.push(
                u8::from(value.label.is_some())
                    | (u8::from(value.amount.is_some()) << 1)
                    | (u8::from(value.score.is_some()) << 2),
            );
            row
        })
        .collect::<Vec<_>>();
    append(before, table, &columns, &records, 15)
}

/// One manual legacy DateTime column from literal OLE day encodings.
/// These raw IEEE bytes are independent of native and expression serializers.
pub fn dates(before: &[u8], table: &str, values: &[Option<f64>]) -> Vec<u8> {
    let columns = [Column {
        name: "Occurred",
        native: 8,
        width: 8,
        variable: None,
    }];
    let records = values
        .iter()
        .map(|value| {
            let mut row = vec![1, 0];
            row.extend(value.unwrap_or(0.0).to_le_bytes());
            row.push(u8::from(value.is_some()));
            row
        })
        .collect::<Vec<_>>();
    append(before, table, &columns, &records, 11)
}
