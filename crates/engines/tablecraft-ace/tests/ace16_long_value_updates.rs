#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_long_value_updates_contract/model.rs"]
mod model;
#[allow(dead_code)]
use tablecraft_testkit::access as support;

use model::*;
use std::io::Cursor;
use tablecraft_ace::{
    Ace16WritePreview, EditError, IndexCheckStatus, InspectError, Int32Update, NativeImage,
    SlotScalarUpdate, WriteBlocker, check_table, check_table_constraints, check_table_long_values,
    read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[test]
fn genuine_empty_memo_index_is_read_checked_before_the_selected_write_guard() {
    for code in [5, 6] {
        let mut bytes = source(code, 0);
        let mut definition = bytes[10 * PAGE..11 * PAGE].to_vec();
        let old = definition.clone();
        definition.fill(0);
        definition[..63].copy_from_slice(&old[..63]);
        // One genuine Memo physical/logical index, with no manufactured numeric
        // entries. Twelve bytes of historical counter precede the three fields.
        definition[75..174].copy_from_slice(&old[63..162]);
        support::put32(&mut definition, 47, 1);
        support::put32(&mut definition, 51, 1);
        definition[111..115].copy_from_slice(&[9, 4, 0, 0]);
        for position in 0..10 {
            support::put16(&mut definition, 178 + position * 3, 0xffff);
        }
        support::put16(&mut definition, 178, 1);
        definition[180] = 1;
        support::put32(&mut definition, 208, (11 << 8) | 6);
        support::put32(&mut definition, 212, 15);
        definition[220] = 0x80;
        let name: Vec<_> = "MemoKey"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        support::put16(&mut definition, 254, name.len() as u16);
        definition[256..270].copy_from_slice(&name);
        definition[270..292].copy_from_slice(&old[162..184]);
        install(&mut bytes, 10, &definition);
        let maps = [
            (0, map(&[ROOT])),
            (0, map(&[ROOT])),
            (0, map(&[MEMO])),
            (0, map(&[MEMO])),
            (0, map(&[OLE])),
            (0, map(&[OLE])),
            (0, map(&[15])),
        ];
        install(&mut bytes, 11, &page(0, &maps));
        let mut leaf = vec![0; PAGE];
        leaf[..2].copy_from_slice(&[4, 1]);
        support::put16(&mut leaf, 2, 3616);
        support::put32(&mut leaf, 4, 10);
        install(&mut bytes, 15, &leaf);

        let checked = check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.schema().columns()[1].native_type(), 12);
        assert_eq!(checked.schema().columns()[1].flags(), 2);
        assert_eq!(checked.schema().columns()[1].storage_length(), 0);
        assert_eq!(checked.indexes().len(), 1);
        assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
        check(&bytes);
        assert_eq!(
            run(&bytes, "Memo", &[]).err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
}

#[test]
fn selected_logical_roots_and_fixed_visitors_skip_an_empty_deleted_hidden_slot() {
    for code in [5, 6] {
        let mut bytes = source(code, 1);
        let live = body(&bytes, ROOT, 0).to_vec();
        let rows = [(0, live), (0xc000, Vec::new()), (0x8000, vec![0xa7; 7])];
        install(&mut bytes, ROOT, &page(10, &rows));
        assert_eq!(word(&bytes, ROOT, 1) & 0x1fff, word(&bytes, ROOT, 0));
        check(&bytes);

        let variable = run(&bytes, "Memo", &[memo_update(Value::Null)]).unwrap();
        assert_eq!(read(variable.candidate_bytes())[0][1], Value::Null);
        assert_eq!(
            word(variable.candidate_bytes(), ROOT, 1),
            word(&bytes, ROOT, 1)
        );
        assert_eq!(
            body(variable.candidate_bytes(), ROOT, 2),
            body(&bytes, ROOT, 2)
        );

        let fixed = capture(&bytes)
            .preview_ace16_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 1, 2)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(read(fixed.candidate_bytes())[0][0], Value::Int32(2));
        assert_eq!(
            word(fixed.candidate_bytes(), ROOT, 1),
            word(&bytes, ROOT, 1)
        );
        assert_eq!(
            body(fixed.candidate_bytes(), ROOT, 2),
            body(&bytes, ROOT, 2)
        );
    }
}
