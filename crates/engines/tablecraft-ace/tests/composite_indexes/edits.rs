use super::*;
use tablecraft_ace::{EditError, Int32Update, NativeImage, WriteBlocker};

#[test]
fn compound_key_changes_stay_blocked_and_unrelated_jet4_fields_preserve_the_index() {
    let mut bytes = fixture::database(
        6,
        &[4, 4, 4],
        &[(0, false), (1, true)],
        &[
            vec![Some(1), Some(2), Some(7)],
            vec![Some(3), Some(4), Some(8)],
        ],
        0,
    );
    fixture::index_flags(&mut bytes, 0x81);
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
    let capture = || {
        NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap()
    };
    assert_eq!(
        capture()
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 1, 5)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
    let plan = capture()
        .plan_int32_updates(
            "Items",
            "K2",
            &[Int32Update::new(0, 7, 17)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.changed_rows(), 1);
    assert_eq!(
        &bytes[14 * 4096..15 * 4096],
        &plan.after_bytes()[14 * 4096..15 * 4096]
    );
    let page = 12 * 4096;
    let row = u16::from_le_bytes([bytes[page + 14], bytes[page + 15]]) as usize;
    let field = page + row + 10..page + row + 14;
    for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
        assert!(before == after || field.contains(&offset));
    }
    assert_eq!(
        numbers(&read(plan.after_bytes()).unwrap()),
        vec![
            vec![Some(1), Some(2), Some(17)],
            vec![Some(3), Some(4), Some(8)]
        ]
    );
}
