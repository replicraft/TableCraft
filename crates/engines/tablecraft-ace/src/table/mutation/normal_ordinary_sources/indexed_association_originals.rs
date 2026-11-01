//! SDK-created originals establish cross-page geometry before AFTER modeling.
use crate::{
    IndexCheckStatus, NativeImage, check_table,
    format::{allocation, reader::Reader},
    inspect_table_index_counter, read_table_index,
    table::mutation::{normal_ordinary, record_placement::space::Space},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(170);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
#[ignore = "requires the two pinned SDK-created association originals"]
fn original_two_page_geometry_and_index_membership_pass_native_admission() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INDEXED_ASSOCIATION_ORIGINAL_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut originals = Vec::new();
    for (header, length, hash, definition, early, late) in [
        (
            5,
            409600,
            "0356754e5fa7ea5ea9040e5112b17c6cec46faff388bda9f65d92d34a4864df5",
            89,
            93,
            95,
        ),
        (
            6,
            479232,
            "abf41a6e18375c84e0f812d682f26ed903145f586a2ee82b6311a215bf2d95d9",
            106,
            110,
            112,
        ),
    ] {
        let bytes =
            fs::read(directory.join(format!("h{header}-association-original.accdb"))).unwrap();
        assert_eq!(bytes.len(), length);
        assert_eq!(digest(&bytes), hash);
        originals.push((bytes, definition, early, late));
    }

    for (bytes, definition, early, late) in originals {
        let image = NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
            .unwrap();
        let source = normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
        let schema = source.table.schema();
        assert_eq!(schema.definition_page(), definition);
        assert_eq!(schema.columns().len(), 4);
        assert_eq!(schema.columns()[3].name(), "Label");
        assert_eq!(schema.columns()[3].storage_length(), 510);
        assert_eq!(schema.columns()[3].extended_flags(), 0);
        let rows = (1..=9)
            .map(|id| {
                vec![
                    Value::Int32(id),
                    Value::Int32(id - 5),
                    Value::Int32((id - 1) % 3),
                    Value::Text("o".repeat(230)),
                ]
            })
            .collect::<Vec<_>>();
        assert_eq!(source.table.rows(), rows);
        assert_eq!(source.records.len(), 9);
        assert_eq!(source.indexes.indexes().len(), 4);

        let mut work = budget();
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget: &mut work,
            cancelled: &|| false,
            reads: 0,
        };
        let owned = allocation::pages(&mut reader, schema.layout.usage).unwrap();
        assert_eq!(owned, [early, late]);
        let space = Space::capture_normal(&image, schema, &owned, &mut work, &|| false).unwrap();
        for (page, count, free) in [(early, 8_u16, 218_u16), (late, 1, 3599)] {
            assert!(
                space
                    .append_eligible(&image, page, &mut work, &|| false)
                    .unwrap()
            );
            let base = page as usize * 4096;
            assert_eq!(&bytes[base..base + 2], &[1, 1]);
            assert_eq!(&bytes[base + 4..base + 8], &definition.to_le_bytes());
            assert_eq!(&bytes[base + 12..base + 14], &count.to_le_bytes());
            assert_eq!(&bytes[base + 2..base + 4], &free.to_le_bytes());
        }
        // Independent 421/27-byte new records plus directory cells must choose
        // later then earlier capacity. This gate creates neither requested row.
        let early_base = early as usize * 4096;
        let late_base = late as usize * 4096;
        let early_free = usize::from(u16::from_le_bytes(
            bytes[early_base + 2..early_base + 4].try_into().unwrap(),
        ));
        let late_free = usize::from(u16::from_le_bytes(
            bytes[late_base + 2..late_base + 4].try_into().unwrap(),
        ));
        let short_bytes = 27 + 2;
        let long_bytes = 421 + 2;
        assert!(short_bytes <= early_free && early_free < long_bytes && long_bytes <= late_free);

        let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(checked.rows(), 9);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        for (name, grouped, history) in [
            ("PrimaryID", false, 9),
            ("PrimaryAlias", false, 9),
            ("GroupOrder", true, 3),
            ("GroupAlias", true, 3),
        ] {
            let mut expected = rows.clone();
            if grouped {
                expected.sort_by_key(|row| match (&row[2], &row[0]) {
                    (Value::Int32(guard), Value::Int32(id)) => (*guard, *id),
                    _ => unreachable!(),
                });
            }
            let indexed = read_table_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(indexed.rows(), expected);
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                "Items",
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value(), history);
        }
        assert_eq!(image.as_bytes(), bytes);
    }
}
