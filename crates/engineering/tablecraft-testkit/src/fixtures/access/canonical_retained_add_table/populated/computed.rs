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

/// A manual nullable large integer named CopiedScore, from literal signed bytes.
/// Used for the query owner's established aggregate carrier, independently of
/// source rows and production declaration/serialization code.
pub fn int64_values(before: &[u8], table: &str, values: &[Option<i64>]) -> Vec<u8> {
    let columns = [Column {
        name: "CopiedScore",
        native: 19,
        width: 8,
        variable: None,
    }];
    let records = values
        .iter()
        .map(|value| {
            let mut row = vec![1, 0];
            row.extend(value.unwrap_or(0).to_le_bytes());
            row.push(u8::from(value.is_some()));
            row
        })
        .collect::<Vec<_>>();
    append(before, table, &columns, &records, 11)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::access::canonical_retained_add_table::fields;

    #[test]
    fn a_fixed_field_after_two_variable_fields_retains_the_literal_ordinary_ordinal() {
        let before = fields::source(fields::Case::FirstNumber).bytes;
        let definition = before.len();
        let image = after(&before, "Archive", &[]);
        // Third 25-byte native descriptor: manual LONG native2/ID2,
        // ordinary variable ordinal2, flags3, fixed offset0 and width4.
        assert_eq!(
            &image[definition + 113..definition + 138],
            &[
                4, 89, 6, 0, 0, 2, 0, 2, 0, 2, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 4, 0
            ]
        );
    }
}
