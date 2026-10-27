#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "long_value_storage_contract/model.rs"]
mod model;
#[allow(dead_code)]
mod support;

use tablecraft_ace::LongValueCheckStatus;
use tablecraft_core::values::Value;

fn compressed(code: u8) -> Vec<u8> {
    let mut bytes = model::image(1, true);
    bytes[20] = code;
    bytes[10 * 4096 + 104] = 1;
    let start = usize::from(
        u16::from_le_bytes(bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap()) & 0x1fff,
    );
    bytes[12 * 4096 + start + 6..12 * 4096 + start + 10].copy_from_slice(&[5, 0, 0, 0x40]);
    let mut page = support::data_page(
        0,
        &[
            (0, vec![0xff, 0xfe, 0x41, 0x42, 0x43]),
            (0, vec![0xf1, 0xe2, 0xd3]),
        ],
    );
    page[4..8].copy_from_slice(b"LVAL");
    support::put16(&mut page, 2, 4096 - 14 - 4 - 8);
    bytes[13 * 4096..14 * 4096].copy_from_slice(&page);
    bytes
}

#[test]
fn physically_compressed_memo_is_read_and_storage_checked_without_preview_feature() {
    for code in [5, 6] {
        let bytes = compressed(code);
        assert_eq!(model::read(&bytes)[0][1], Value::Text("ABC".into()));
        let checked = model::check(&bytes).unwrap();
        let column = &checked.columns()[0];
        assert_eq!(column.status(), LongValueCheckStatus::Checked);
        assert_eq!(column.values().single(), 1);
        assert_eq!(column.distinct_chunks(), 1);
        assert_eq!(column.orphan_rows(), 1);
        assert_eq!(bytes[10 * 4096 + 104], 1);
    }
}

#[test]
fn ordinary_profile_rejects_ole_compression_and_unrecognized_memo_bits() {
    for code in [5, 6] {
        for (native, extended) in [
            (11, 1),
            (12, 2),
            (12, 3),
            (12, 0x10),
            (12, 0x80),
            (12, 0xc1),
        ] {
            let mut bytes = compressed(code);
            bytes[10 * 4096 + 88] = native;
            bytes[10 * 4096 + 104] = extended;
            assert_eq!(
                model::check(&bytes).unwrap().columns()[0].status(),
                LongValueCheckStatus::UnsupportedColumn
            );
        }
    }
}
