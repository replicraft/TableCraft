//! Independent indexed DELETE source reads; no DELETE planner, serializer or journal.
use crate::{
    IndexCheckStatus, NativeImage, PayloadPresence, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(163);
const ROWS: [[i32; 3]; 3] = [[1, -1, 0], [2, 0, 1], [3, 1, 2]];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn entry(value: i32, data: u32, slot: usize) -> Vec<u8> {
    let mut raw = vec![127];
    raw.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
    raw.extend_from_slice(&((data << 8) | slot as u32).to_be_bytes());
    raw
}

fn read_view(bytes: &[u8], definition: u32, maps: u32, roots: [u32; 2], retained: &[usize]) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    assert_eq!(table.schema().declared_rows() as usize, retained.len());
    assert_eq!(table.columns().len(), 3);
    for (field, name) in table
        .schema()
        .columns()
        .iter()
        .zip(["Id", "Score", "Guard"])
    {
        assert_eq!(field.name(), name);
        assert_eq!(
            (
                field.native_type(),
                field.flags(),
                field.extended_flags(),
                field.storage_length()
            ),
            (4, 3, 0, 4)
        );
    }
    let expected = retained
        .iter()
        .map(|slot| {
            ROWS[*slot]
                .into_iter()
                .map(Value::Int32)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(table.rows(), expected);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), retained.len());
    assert_eq!(checked.indexes().len(), 4);
    for (checked, (name, physical, column, kind, flags)) in checked.indexes().iter().zip([
        ("PrimaryID", 0, 0, 1, 137),
        ("PrimaryAlias", 0, 0, 0, 137),
        ("GroupOrder", 1, 2, 0, 128),
        ("GroupAlias", 1, 2, 0, 128),
    ]) {
        assert_eq!(checked.status(), IndexCheckStatus::Checked);
        let index = checked.index();
        assert_eq!(index.name(), name);
        assert_eq!(index.data_number(), physical);
        assert_eq!(index.root_page(), roots[physical as usize]);
        assert_eq!(index.usage_map(), (maps << 8) | (2 + physical));
        assert_eq!((index.native_type(), index.flags()), (kind, flags));
        assert_eq!(index.columns().len(), 1);
        assert_eq!(index.columns()[0].number(), column);
        assert!(!index.columns()[0].descending());
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.physical_index(), physical);
        assert_eq!(counter.value(), 3);
        let ordered = read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        // Both independent source key columns are strictly ascending in the
        // original ROW order. Surviving logical ordinals may compact, ROWs do not.
        assert_eq!(ordered.rows(), expected);
    }
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(constraints.rows(), retained.len());
    let coverage = image
        .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    image
        .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
#[ignore = "requires independently constructed indexed DELETE source directory"]
fn genuine_indexed_delete_literals_read_all_aliases_and_preserve_complete_complement() {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_INDEXED_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut seen = Vec::new();
    for (
        header,
        recipe,
        definition,
        data,
        maps,
        roots,
        before_file,
        after_file,
        original_hash,
        after_hash,
    ) in [
        (
            5_u8,
            "first",
            89_u32,
            93_u32,
            90_u32,
            [91_u32, 92_u32],
            "h5-indexed-delete-first-before.accdb",
            "h5-indexed-delete-first-deleted.accdb",
            "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            "aca0a0c8d49867e680f6fed07d26e6a9e3c2bafdc9968d7bce30cbe42a9c27b2",
        ),
        (
            5_u8,
            "middle",
            89_u32,
            93_u32,
            90_u32,
            [91_u32, 92_u32],
            "h5-indexed-delete-middle-before.accdb",
            "h5-indexed-delete-middle-deleted.accdb",
            "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            "c9fc6bd2a25ea403e5baa3464002fb6f8c2bcd9de44bf1b5ef2fc3091203ae4d",
        ),
        (
            5_u8,
            "exterior",
            89_u32,
            93_u32,
            90_u32,
            [91_u32, 92_u32],
            "h5-indexed-delete-exterior-before.accdb",
            "h5-indexed-delete-exterior-deleted.accdb",
            "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            "9f16d86877ce3e0430bd3d7b3161099f3b51d508932f396b932a94b4ccdb039c",
        ),
        (
            5_u8,
            "all",
            89_u32,
            93_u32,
            90_u32,
            [91_u32, 92_u32],
            "h5-indexed-delete-all-before.accdb",
            "h5-indexed-delete-all-deleted.accdb",
            "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            "1aa761e24fabbab4760086a8271b2075efdeac4e19aeb6c0aad80627877a1885",
        ),
        (
            6_u8,
            "first",
            106_u32,
            110_u32,
            107_u32,
            [108_u32, 109_u32],
            "h6-indexed-delete-first-before.accdb",
            "h6-indexed-delete-first-deleted.accdb",
            "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            "b77fb1b76fdddd9334a95826ad3c3ca367d41c588c151aca38393386bc251824",
        ),
        (
            6_u8,
            "middle",
            106_u32,
            110_u32,
            107_u32,
            [108_u32, 109_u32],
            "h6-indexed-delete-middle-before.accdb",
            "h6-indexed-delete-middle-deleted.accdb",
            "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            "90850fe6764b7bc140487658c4f048369fd1c8ad56faa4f0759ea16b7ac02bcc",
        ),
        (
            6_u8,
            "exterior",
            106_u32,
            110_u32,
            107_u32,
            [108_u32, 109_u32],
            "h6-indexed-delete-exterior-before.accdb",
            "h6-indexed-delete-exterior-deleted.accdb",
            "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            "cb3507d404d487af308f77f3ce02f33759d876428f09eac25289caf590ea75c9",
        ),
        (
            6_u8,
            "all",
            106_u32,
            110_u32,
            107_u32,
            [108_u32, 109_u32],
            "h6-indexed-delete-all-before.accdb",
            "h6-indexed-delete-all-deleted.accdb",
            "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            "185e81274647b9f4ef79b51ba4ae830095d553ce2fa0b40bcc26ff00b971cf2e",
        ),
    ] {
        let selected: &[usize] = match recipe {
            "first" => &[0],
            "middle" => &[1],
            "exterior" => &[0, 2],
            "all" => &[0, 1, 2],
            _ => panic!("unexpected literal recipe"),
        };
        seen.push((header, recipe.to_owned()));
        let retained = (0..3)
            .filter(|slot| !selected.contains(slot))
            .collect::<Vec<_>>();
        let before = fs::read(directory.join(before_file)).unwrap();
        let after = fs::read(directory.join(after_file)).unwrap();
        assert_eq!(hash(&before), original_hash);
        assert_eq!(hash(&after), after_hash);
        assert_eq!(before.len(), after.len());
        read_view(&before, definition, maps, roots, &[0, 1, 2]);
        read_view(&after, definition, maps, roots, &retained);
        let td = definition as usize * 4096;
        let base = data as usize * 4096;
        let mut expected = before.clone();
        assert_eq!(&before[td + 16..td + 20], &[3, 0, 0, 0]);
        expected[td + 16..td + 20].copy_from_slice(&(retained.len() as u32).to_le_bytes());
        for (slot, values) in ROWS.iter().enumerate() {
            let start = 4081 - slot * 15;
            let mut record = vec![3, 0];
            for value in values {
                record.extend_from_slice(&value.to_le_bytes());
            }
            record.push(7);
            assert_eq!(&before[base + start..base + start + 15], record);
            let original = start as u16;
            let at = base + 14 + slot * 2;
            assert_eq!(&before[at..at + 2], original.to_le_bytes());
            if selected.contains(&slot) {
                expected[at..at + 2].copy_from_slice(&(original | 0xc000).to_le_bytes());
            }
        }
        for (physical, root) in roots.into_iter().enumerate() {
            let page = root as usize * 4096;
            let column = if physical == 0 { 0 } else { 2 };
            assert_eq!(&before[page..page + 2], &[4, 1]);
            assert_eq!(&before[page + 2..page + 4], &3589_u16.to_le_bytes());
            assert_eq!(&before[page + 4..page + 8], &definition.to_le_bytes());
            assert_eq!(&before[page + 8..page + 27], &[0; 19]);
            let original_keys = (0..3)
                .flat_map(|slot| entry(ROWS[slot][column], data, slot))
                .collect::<Vec<_>>();
            assert_eq!(&before[page + 480..page + 507], original_keys);
            let mut original_bitmap = [0_u8; 453];
            for end in [9, 18, 27] {
                original_bitmap[end / 8] |= 1 << (end % 8);
            }
            assert_eq!(&before[page + 27..page + 480], &original_bitmap);
            let keys = retained
                .iter()
                .flat_map(|slot| entry(ROWS[*slot][column], data, *slot))
                .collect::<Vec<_>>();
            expected[page + 2..page + 4]
                .copy_from_slice(&((3616 - keys.len()) as u16).to_le_bytes());
            expected[page + 27..page + 480].fill(0);
            for end in (9..=keys.len()).step_by(9) {
                expected[page + 27 + end / 8] |= 1 << (end % 8);
            }
            expected[page + 480..page + 480 + keys.len()].copy_from_slice(&keys);
            assert_eq!(
                &after[page + 480 + keys.len()..page + 4096],
                &before[page + 480 + keys.len()..page + 4096]
            );
        }
        assert_eq!(after, expected, "complete literal indexed DELETE image");
    }
    seen.sort();
    let mut expected = Vec::new();
    for header in [5, 6] {
        for recipe in ["all", "exterior", "first", "middle"] {
            expected.push((header, recipe.to_owned()));
        }
    }
    assert_eq!(seen, expected);
}
