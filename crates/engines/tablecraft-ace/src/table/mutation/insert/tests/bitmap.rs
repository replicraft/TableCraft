//! Public INSERT and full final audit through ordinary-candidate map promotion.
use super::model;
use crate::RowInsert;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::insert_seed as source_model;

const PAGE: usize = 4096;

fn bitmap(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut bytes = vec![0; PAGE];
    bytes[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in members {
        assert!(number < 32736);
        bytes[4 + number / 8] |= 1 << (number % 8);
    }
    bytes
}

fn indirect(number: u32) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    bytes[0] = 1;
    bytes[1..5].copy_from_slice(&number.to_le_bytes());
    bytes
}

fn inline(members: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for &number in members {
        assert!(number < 512);
        bytes[5 + number / 8] |= 1 << (number % 8);
    }
    bytes
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn source(code: u8, aliases: bool) -> Vec<u8> {
    let mut bytes = source_model::source(code, aliases, true, None);
    bytes.resize(600 * PAGE, 0);
    let mut deleted = support::data_page(10, &[(0x8000, vec![0x9d; 4000])]);
    deleted[2..4].copy_from_slice(&80_u16.to_le_bytes());
    install(&mut bytes, 15, &deleted);
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, indirect(597)), (0, inline(&[12, 15]))]),
    );
    install(&mut bytes, 597, &bitmap([12, 15]));
    install(&mut bytes, 598, &bitmap([15]));
    install(&mut bytes, 1, &support::data_page(1, &[(0, indirect(599))]));
    install(&mut bytes, 599, &bitmap(600..32736));
    bytes
}

fn expected(before: &[u8], payload: &[u8]) -> Vec<u8> {
    assert_eq!(payload.len(), 255);
    let mut bytes = before.to_vec();
    bytes.resize(602 * PAGE, 0);

    let mut row = vec![2, 0, 0x11, 0x27, 0, 0];
    row.extend_from_slice(payload);
    row.extend([5, 1, 6, 0, 1, 0, 3]);
    assert_eq!(row.len(), 268);
    let mut data = vec![0; PAGE];
    data[..2].copy_from_slice(&[1, 1]);
    data[2..4].copy_from_slice(&3812_u16.to_le_bytes());
    data[4..8].copy_from_slice(&10_u32.to_le_bytes());
    data[12..14].copy_from_slice(&1_u16.to_le_bytes());
    data[14..16].copy_from_slice(&3828_u16.to_le_bytes());
    data[3828..].copy_from_slice(&row);
    install(&mut bytes, 600, &data);

    // Highest 15 failed once; lower 12 survives without a second fit attempt.
    // The full map row changes grammar. Metadata 601 is not an ordinary member.
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, indirect(597)), (0, indirect(601))]),
    );
    install(&mut bytes, 597, &bitmap([12, 15, 600]));
    install(&mut bytes, 601, &bitmap([12, 600]));
    install(&mut bytes, 599, &bitmap(602..32736));

    // Retain all original primary entries and suffix residue. The added full
    // non-NULL key encodes ID 10001 and visible logical ROW 600:0.
    let root = 13 * PAGE;
    bytes[root + 507..root + 516].copy_from_slice(&[127, 128, 0, 39, 17, 0, 2, 88, 0]);
    bytes[root + 31] |= 1 << 4;
    bytes[root + 2..root + 4].copy_from_slice(&3580_u16.to_le_bytes());
    bytes[10 * PAGE + 16..10 * PAGE + 20].copy_from_slice(&4_u32.to_le_bytes());
    bytes[10 * PAGE + 67..10 * PAGE + 71].copy_from_slice(&18_u32.to_le_bytes());
    assert_eq!(
        &bytes[root + 516..root + PAGE],
        &before[root + 516..root + PAGE]
    );
    assert_eq!(&bytes[12 * PAGE..13 * PAGE], &before[12 * PAGE..13 * PAGE]);
    assert_eq!(&bytes[15 * PAGE..16 * PAGE], &before[15 * PAGE..16 * PAGE]);
    assert_eq!(
        &bytes[598 * PAGE..599 * PAGE],
        &before[598 * PAGE..599 * PAGE]
    );
    assert_eq!(bytes[597 * PAGE + 4 + 601 / 8] & 2, 0);
    assert_eq!(bytes[601 * PAGE + 4 + 601 / 8] & 2, 0);
    bytes
}

#[test]
fn public_insert_promotes_hint_bitmap_without_claiming_metadata_as_data() {
    for code in [5, 6] {
        for aliases in [false, true] {
            let before = source(code, aliases);
            let indexes = if aliases { 2 } else { 1 };
            model::checked(&before, 3, indexes);
            let payload = (0..255).collect::<Vec<u8>>();
            let expected = expected(&before, &payload);
            model::checked(&expected, 4, indexes);

            let candidate = model::capture(&before)
                .preview_ace16_insert_rows(
                    "Items",
                    &[RowInsert::new(vec![
                        Value::Int32(10001),
                        Value::Binary(payload),
                    ])],
                    &mut model::budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.changed_rows(), 1);
            assert_eq!(candidate.candidate_bytes(), expected);
            model::checked(candidate.candidate_bytes(), 4, indexes);
        }
    }
}
