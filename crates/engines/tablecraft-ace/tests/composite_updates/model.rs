use super::*;

pub fn name(column: usize) -> &'static str {
    [
        "Id", "Score", "K2", "K3", "K4", "K5", "K6", "K7", "K8", "K9",
    ][column]
}
pub fn width(kind: u8) -> usize {
    match kind {
        2 => 1,
        3 => 2,
        4 => 4,
        0x13 | 5 => 8,
        _ => panic!("fixture carrier"),
    }
}
pub fn value(kind: u8, key: Option<i64>) -> Value {
    key.map_or(Value::Null, |key| match kind {
        2 => Value::Byte(key.try_into().unwrap()),
        3 => Value::Int16(key.try_into().unwrap()),
        4 => Value::Int32(key.try_into().unwrap()),
        0x13 => Value::Int64(key),
        5 => Value::Currency(Currency::from_scaled(key)),
        _ => panic!("fixture carrier"),
    })
}
pub fn with_alias(mut bytes: Vec<u8>, count: usize) -> Vec<u8> {
    let logical = 75 + 25 * count + (0..count).map(|i| 2 + name(i).len() * 2).sum::<usize>() + 52;
    let page = &mut bytes[10 * PAGE_BYTES..11 * PAGE_BYTES];
    page.copy_within(logical + 28..logical + 28 + 18, logical + 56);
    page.copy_within(logical..logical + 28, logical + 28);
    support::put32(page, logical + 28 + 4, 1);
    support::put32(page, 47, 2);
    let text = "Alias"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    support::put16(page, logical + 56 + 18, text.len() as u16);
    page[logical + 56 + 20..logical + 56 + 20 + text.len()].copy_from_slice(&text);
    bytes
}
pub fn verify(
    before: &[u8],
    after: &[u8],
    kinds: &[u8],
    order: &[(usize, bool)],
    rows: &[Vec<Option<i64>>],
    selected: usize,
    edited: &[usize],
) {
    let mut sorted = rows.iter().enumerate().collect::<Vec<_>>();
    sorted.sort_by(|(a, x), (b, y)| {
        for &(column, descending) in order {
            let cmp = x[column].cmp(&y[column]);
            if !cmp.is_eq() {
                return if descending { cmp.reverse() } else { cmp };
            }
        }
        a.cmp(b)
    });
    let expected = sorted
        .iter()
        .map(|(_, row)| {
            row.iter()
                .zip(kinds)
                .map(|(key, kind)| value(*kind, *key))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let report = check_table(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for index in report.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        let table = read_table_index(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            index.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows(), &expected);
    }
    assert_eq!(before.len(), after.len());
    let mut allowed = vec![false; before.len()];
    let payload = 2 + kinds[..selected]
        .iter()
        .map(|kind| width(*kind))
        .sum::<usize>();
    let total = 2 + kinds.iter().map(|kind| width(*kind)).sum::<usize>();
    for &ordinal in edited {
        let row = 12 * PAGE_BYTES
            + (u16::from_le_bytes(
                before[12 * PAGE_BYTES + 14 + ordinal * 2..12 * PAGE_BYTES + 16 + ordinal * 2]
                    .try_into()
                    .unwrap(),
            ) as usize
                & 0x1fff);
        let mask = row + total + selected / 8;
        let bit = 1 << (selected % 8);
        assert_eq!((before[mask] ^ after[mask]) & !bit, 0);
        allowed[mask] = true;
        if rows[ordinal][selected].is_none() {
            assert_eq!(
                &before[row + payload..row + payload + width(kinds[selected])],
                &after[row + payload..row + payload + width(kinds[selected])]
            );
        } else {
            allowed[row + payload..row + payload + width(kinds[selected])].fill(true);
        }
    }
    for (number, page) in before.as_chunks::<PAGE_BYTES>().0.iter().enumerate() {
        if matches!(page[0], 3 | 4) && u32::from_le_bytes(page[4..8].try_into().unwrap()) == 10 {
            let at = number * PAGE_BYTES;
            let old = PAGE_BYTES - u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
            let new =
                PAGE_BYTES - u16::from_le_bytes(after[at + 2..at + 4].try_into().unwrap()) as usize;
            allowed[at + 2..at + 4].fill(true);
            allowed[at + 24..at + 26].fill(true);
            allowed[at + 27..at + old.max(new)].fill(true);
        }
    }
    assert!(
        before
            .iter()
            .zip(after)
            .zip(allowed)
            .all(|((&a, &b), allowed)| a == b || allowed)
    );
}
