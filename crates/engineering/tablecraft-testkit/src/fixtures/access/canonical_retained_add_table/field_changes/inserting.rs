//! Literal fresh records after middle-field retirement, without deleted-field inference.
use crate::fixtures::access::{
    generated_record_insert::placement::{self, Pages},
    packed_record::append_record,
    put32,
};

const PAGE: usize = 4096;

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn field(bytes: &[u8], at: usize, native: u8, number: u16, offset: u16, width: u16, flags: u8) {
    assert_eq!(bytes[at], native);
    assert_eq!(&bytes[at + 5..at + 7], &number.to_le_bytes());
    assert_eq!(&bytes[at + 15..at + 17], &[flags, 0]);
    assert_eq!(&bytes[at + 21..at + 23], &offset.to_le_bytes());
    assert_eq!(&bytes[at + 23..at + 25], &width.to_le_bytes());
}

/// Append one resolved nullable Score outcome to an ordinary keyless schema.
/// The retired native0 value/default never transfers to native1. Its absent
/// default resolves to None; a caller with a different qualified default must
/// supply that actual resolved outcome explicitly.
/// LAST, every original record and all unrelated pages remain byte-identical.
pub fn keyless_score(before: &[u8], definition: u32, score: Option<i32>) -> Vec<u8> {
    let base = definition as usize * PAGE;
    assert_eq!(&before[base + 41..base + 47], &[2, 0, 0, 0, 1, 0]);
    assert_eq!(&before[base + 47..base + 55], &[0; 8]);
    field(before, base + 63, 4, 1, 4, 4, 3);
    let data = (definition as usize + 1) * PAGE;
    assert_eq!(&before[data + 4..data + 8], &definition.to_le_bytes());
    assert!(u8::try_from(word(before, data + 12)).is_ok());
    let mut row = vec![2, 0, 0, 0, 0, 0];
    row.extend(score.unwrap_or(0).to_le_bytes());
    row.push(if score.is_some() { 2 } else { 0 });
    // NO_CLONE: The independent complete AFTER must coexist with the caller's
    // unchanged BEFORE while only the literal append/count bytes are replaced.
    let mut after = before.to_vec();
    append_record(&mut after[data..data + PAGE], &row);
    let live = u32::from_le_bytes(before[base + 16..base + 20].try_into().unwrap());
    put32(&mut after, base + 16, live.checked_add(1).unwrap());
    after
}

/// Append generated ID0 and resolved nullable Currency2 after Score1 retires.
/// Some(0) is the declared Currency type default; None is explicit NULL.
/// The retired fixed hole is zero and its native1 presence bit stays clear.
pub fn currency(before: &[u8], definition: u32, amount: Option<i64>) -> Vec<u8> {
    let base = definition as usize * PAGE;
    assert_eq!(&before[base + 41..base + 47], &[3, 0, 0, 0, 2, 0]);
    assert_eq!(&before[base + 47..base + 55], &[1, 0, 0, 0, 1, 0, 0, 0]);
    field(before, base + 75, 4, 0, 0, 4, 7);
    field(before, base + 100, 5, 2, 8, 8, 3);
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut row = vec![3, 0];
    row.extend(id.to_le_bytes());
    row.extend([0; 4]);
    row.extend(amount.unwrap_or(0).to_le_bytes());
    row.push(if amount.is_some() { 5 } else { 1 });
    placement::append(before, pages, id, &row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::access::{
        canonical_retained_add_table::{field_changes, fields, typed_fields},
        raw_forwarding_chain,
    };

    #[test]
    fn keyless_rows_keep_retired_zero_clear_and_leave_last_and_unrelated_pages_untouched() {
        let seed = fields::source(fields::Case::FirstNumber).bytes;
        let definition = (seed.len() / 4096) as u32;
        let catalog =
            u16::from_le_bytes(seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap()) as usize;
        let populated = field_changes::keyless_first_field(&seed, "Keyless");
        let before = field_changes::drop_field(&populated, definition, catalog, "Id");
        let null = keyless_score(&before, definition, None);
        assert_eq!(
            raw_forwarding_chain::record(&null, definition + 1, 1),
            &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        let supplied = keyless_score(&null, definition, Some(77));
        assert_eq!(
            raw_forwarding_chain::record(&supplied, definition + 1, 2),
            &[2, 0, 0, 0, 0, 0, 77, 0, 0, 0, 2]
        );
        assert_eq!(
            raw_forwarding_chain::record(&supplied, definition + 1, 0),
            raw_forwarding_chain::record(&before, definition + 1, 0)
        );
        let base = definition as usize * 4096;
        assert_eq!(&supplied[base..base + 16], &before[base..base + 16]);
        assert_eq!(&supplied[base + 16..base + 20], &[3, 0, 0, 0]);
        assert!(
            &supplied[base + 20..base + 4096] == &before[base + 20..base + 4096],
            "keyless append preserves every declaration and counter beyond live count"
        );
        for page in 0..before.len() / 4096 {
            if page != definition as usize && page != definition as usize + 1 {
                assert!(
                    &supplied[page * 4096..(page + 1) * 4096]
                        == &before[page * 4096..(page + 1) * 4096],
                    "protected page {page}"
                );
            }
        }
    }

    #[test]
    fn generated_currency_rows_keep_the_retired_score_bit_clear_for_default_and_null() {
        let source = fields::source(fields::Case::FirstNumber);
        let added = typed_fields::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            "Amount",
            typed_fields::Kind::Currency,
        );
        let populated = typed_fields::update_currency(&added, source.definition, 0, Some(12345));
        let before =
            field_changes::drop_field(&populated, source.definition, source.catalog_root, "Score");
        let base = source.definition as usize * 4096;
        assert_eq!(&before[base + 20..base + 24], &[2, 0, 0, 0]);
        let defaulted = currency(&before, source.definition, Some(0));
        assert_eq!(
            raw_forwarding_chain::record(&defaulted, source.definition + 1, 2),
            &[3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]
        );
        let null = currency(&defaulted, source.definition, None);
        assert_eq!(
            raw_forwarding_chain::record(&null, source.definition + 1, 3),
            &[3, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(
            raw_forwarding_chain::record(&null, source.definition + 1, 0),
            raw_forwarding_chain::record(&before, source.definition + 1, 0)
        );
        assert_eq!(&null[base + 20..base + 24], &[4, 0, 0, 0]);
        assert_eq!(&null[base + 41..base + 47], &before[base + 41..base + 47]);
        for page in 0..before.len() / 4096 {
            if !(source.definition as usize..source.definition as usize + 3).contains(&page) {
                assert!(
                    &null[page * 4096..(page + 1) * 4096]
                        == &before[page * 4096..(page + 1) * 4096],
                    "protected page {page}"
                );
            }
        }
    }
}
