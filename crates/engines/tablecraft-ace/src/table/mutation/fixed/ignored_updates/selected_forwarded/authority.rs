//! Physical targets must have one logical owner and remain in the owned map.
use super::{PAGE, budget, capture, case, model, span, updates};
use crate::{
    EditError, IndexCheckStatus, InspectError, WriteBlocker, check_table, inspect_table_schema,
    read_table,
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn bad_target(alias: bool) -> Vec<u8> {
    let mut before = case(6, false, 2, false, Some(30), false).before;
    if alias {
        let root = span(&before, 12, 0);
        before[root.start..root.start + 4].copy_from_slice(&(16_u32 << 8).to_le_bytes());
        let at = 12 * PAGE + 14;
        let word = u16::from_le_bytes(before[at..at + 2].try_into().unwrap());
        before[at..at + 2].copy_from_slice(&(word | 0x4000).to_le_bytes());
        // Make both physical indexes genuinely ordinary/nonunique so duplicate
        // visible IDs are reader/check-valid. Their logical aliases stay real.
        before[10 * PAGE + 238] = 0x80;
        before[10 * PAGE + 319] = 0;
        let mut primary = [(3_u32, 0_u32), (2, 1), (3, 2)]
            .into_iter()
            .map(|(identity, slot)| {
                let mut key = vec![127];
                key.extend((identity ^ 0x8000_0000).to_be_bytes());
                key.extend(((12_u32 << 8) | slot).to_be_bytes());
                key
            })
            .collect::<Vec<_>>();
        primary.sort();
        before[13 * PAGE..14 * PAGE].copy_from_slice(&model::leaf(None, &primary, 0, 0, 0));
        let mut group = vec![127];
        group.extend((10_u32 ^ 0x8000_0000).to_be_bytes());
        group.extend(((12_u32 << 8) | 1).to_be_bytes());
        before[14 * PAGE..15 * PAGE].copy_from_slice(&model::leaf(None, &[group], 0, 0, 0));
    } else {
        model::map_page(&mut before, 11, vec![model::map([12]), model::map([12])]);
    }
    before
}

#[test]
fn qualify_selected_forwarded_alias_and_unowned_sources_are_reader_valid() {
    for alias in [false, true] {
        let before = bad_target(alias);
        let image = capture(&before);
        let table = read_table(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows().len(), 3);
        // The reader may resolve this layout, but shared or unowned storage is
        // not authority to write the selected final record.
        assert_eq!(table.rows()[2][1], tablecraft_core::values::Value::Null);
        let checked = check_table(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
    }
}

#[test]
fn shared_or_unowned_final_targets_refuse_before_any_patch_callback() {
    for alias in [false, true] {
        let before = bad_target(alias);
        let image = capture(&before);
        let schema = inspect_table_schema(
            &mut image.reader(),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let callbacks = Cell::new(0);
        let result = crate::table::mutation::rows::final_record::visit(
            &image,
            &schema,
            &updates(2, Some(30)),
            &mut budget(),
            &|| false,
            |_, _| {
                callbacks.set(callbacks.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result, Err(EditError::Unsupported(WriteBlocker::RowLayout)));
        assert_eq!(callbacks.get(), 0);
        assert_eq!(
            capture(&before)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Group",
                    &updates(2, Some(30)),
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

#[test]
fn selected_forwarded_preview_observes_each_resource_pool() {
    let case = case(6, false, 2, true, Some(30), false);
    let input = updates(2, Some(30));
    for mut limits in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            capture(&case.before).preview_ace16_fixed_updates(
                "Items",
                "Group",
                &input,
                &mut limits,
                &|| false,
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
}
