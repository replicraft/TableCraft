//! Mixed same-page/cross-page source reads before any writer or planner.
use crate::format::{allocation, reader::Reader};
use crate::{
    LongValueCheckStatus, NativeImage, PayloadPresence, PropertyGroupKind, check_table_long_values,
    inspect_table_indexes, inspect_table_properties, read_table,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(149);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_view(bytes: &[u8], definition: u32, maps: u32, after: bool) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    assert_eq!(table.columns().len(), 3);
    let markers = if after {
        ["aaaaaaaaaaaaaaaaaaa", "ccccccccccccccccccc"]
    } else {
        ["unchanged sentinel", "unchanged sentinel"]
    };
    assert_eq!(
        table.rows(),
        [
            vec![
                Value::Int32(23),
                Value::Text(markers[0].into()),
                Value::Binary(vec![199, 0, 255])
            ],
            vec![
                Value::Int32(24),
                Value::Text(markers[1].into()),
                Value::Binary(vec![199, 0, 255])
            ],
        ]
    );
    let fields = table.schema().columns();
    assert_eq!(
        (
            fields[0].name(),
            fields[0].native_type(),
            fields[0].flags(),
            fields[0].storage_length()
        ),
        ("ID", 4, 3, 4)
    );
    assert_eq!(
        (
            fields[1].name(),
            fields[1].native_type(),
            fields[1].flags(),
            fields[1].extended_flags(),
            fields[1].storage_length()
        ),
        ("Marker", 10, 2, 0, 80)
    );
    assert_eq!(
        (
            fields[2].name(),
            fields[2].native_type(),
            fields[2].flags(),
            fields[2].extended_flags(),
            fields[2].storage_length()
        ),
        ("Bytes", 11, 2, 0, 0)
    );
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(indexes.indexes().is_empty());
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let table_groups = properties
        .groups()
        .iter()
        .filter(|group| group.kind() == PropertyGroupKind::Table)
        .collect::<Vec<_>>();
    assert_eq!(table_groups.len(), 1);
    assert_eq!(table_groups[0].properties().len(), 1);
    let property = table_groups[0].get("PublishToWeb").unwrap();
    assert_eq!(
        (
            property.native_type(),
            property.flags(),
            property.raw_value()
        ),
        (2, 0, &[0][..])
    );
    let lval = check_table_long_values(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(lval.rows(), 2);
    assert_eq!(lval.columns().len(), 1);
    let bytes_column = &lval.columns()[0];
    assert_eq!(bytes_column.column_number(), 2);
    assert_eq!(bytes_column.status(), LongValueCheckStatus::Checked);
    let references = bytes_column.maps().unwrap();
    assert_eq!(
        (references.owned(), references.candidate()),
        ((maps << 8) | 2, (maps << 8) | 3)
    );
    assert_eq!(
        (
            bytes_column.owned_pages(),
            bytes_column.candidate_pages(),
            bytes_column.owned_rows(),
            bytes_column.distinct_chunks()
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (
            bytes_column.values().nulls(),
            bytes_column.values().inline(),
            bytes_column.values().single(),
            bytes_column.values().chained()
        ),
        (0, 2, 0, 0)
    );
    let coverage = image
        .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    // Readonly full normal admission includes original constraints/properties,
    // complete selected declaration tails and physical/LVAL exclusion. It does
    // not stage a row, evaluate a program or return a writable plan.
    image
        .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

fn memberships(raw: &[u8], maps: u32, old: u32, new: u32) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let mut bytes = image.reader();
    let mut work = budget();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget: &mut work,
        cancelled: &|| false,
        reads: 0,
    };
    assert_eq!(
        allocation::pages(&mut reader, maps << 8).unwrap(),
        [old, new]
    );
    assert_eq!(
        allocation::pages(&mut reader, (maps << 8) | 1).unwrap(),
        [new]
    );
    let global = allocation::RepresentedMap::read(
        &mut reader,
        allocation::GLOBAL_FREE_REFERENCE,
        allocation::MapRole::GlobalFree,
    )
    .unwrap();
    global.require_allocated(&mut reader, old).unwrap();
    global.require_allocated(&mut reader, new).unwrap();
}

#[test]
#[ignore = "requires explicit frozen alternate BEFORE inputs and authored mixed-event outputs"]
fn genuine_mixed_page_event_source_and_literal_after_read_before_any_writer() {
    let parent =
        PathBuf::from(std::env::var_os("TABLECRAFT_ALTERNATE_OWNED_EXPECTED_DIR").unwrap());
    let outputs = PathBuf::from(std::env::var_os("TABLECRAFT_MIXED_OWNED_EXPECTED_DIR").unwrap());
    assert!(parent.is_absolute() && outputs.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, old, new) in [
        (
            5_u8,
            "a8954a42742b6414726055970c628c88436059d7e14f919d939cae95d35c4cbd",
            107_u32,
            108_u32,
            110_u32,
            112_u32,
        ),
        (
            6_u8,
            "a0b3bf39db8322a21b90cde1c72d168f1ece059c268fae6212eb93c4692b0a21",
            125_u32,
            126_u32,
            127_u32,
            129_u32,
        ),
    ] {
        let path = parent.join(format!("h{header}-alternate-owned-before.accdb"));
        let alternate = fs::read(&path).unwrap();
        assert_eq!(hash(&alternate), digest);
        assert_eq!(alternate.len(), (new as usize + 1) * 4096);
        let td = definition as usize * 4096;
        let old = old as usize * 4096;
        let next = new as usize * 4096;
        assert_eq!(&alternate[td + 16..td + 20], &[1, 0, 0, 0]);
        assert_eq!(&alternate[old + 12..old + 16], &[255, 0, 190, 15]);
        assert_eq!(&alternate[next + 2..next + 4], &[242, 15]);
        assert_eq!(&alternate[next + 12..next + 16], &[0; 4]);
        let first = &alternate[old + 4030..old + 4096];
        assert_eq!(first.len(), 66);
        assert_eq!(&first[..6], &[3, 0, 23, 0, 0, 0]);
        assert_eq!(
            &first[42..57],
            &[3, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 199, 0, 255]
        );
        let mut second = first.to_vec();
        second[2..6].copy_from_slice(&24_i32.to_le_bytes());
        let mut before = alternate.clone();
        before[td + 16..td + 20].copy_from_slice(&2_u32.to_le_bytes());
        before[next + 2..next + 4].copy_from_slice(&4014_u16.to_le_bytes());
        before[next + 12..next + 16].copy_from_slice(&[1, 0, 190, 15]);
        before[next + 4030..next + 4096].copy_from_slice(&second);
        let actual_before =
            fs::read(outputs.join(format!("h{header}-mixed-events-before.accdb"))).unwrap();
        assert_eq!(actual_before, before);
        read_view(&actual_before, definition, maps, false);
        memberships(&actual_before, maps, (old / 4096) as u32, new);

        let mut cross = first[..6].to_vec();
        let mut same = second[..6].to_vec();
        for _ in 0..19 {
            cross.extend_from_slice(&[97, 0]);
            same.extend_from_slice(&[99, 0]);
        }
        for record in [&mut cross, &mut same] {
            record.extend_from_slice(&first[42..57]);
            record.extend_from_slice(&[59, 0, 44, 0, 6, 0, 2, 0, 7]);
            assert_eq!(record.len(), 68);
        }
        let mut after = before.clone();
        after[next + 3962..next + 4030].copy_from_slice(&cross);
        after[next + 3894..next + 3962].copy_from_slice(&same);
        after[next + 2..next + 4].copy_from_slice(&3874_u16.to_le_bytes());
        after[next + 12..next + 20].copy_from_slice(&[3, 0, 190, 79, 122, 143, 54, 143]);
        after[old + 14..old + 16].copy_from_slice(&[190, 79]);
        after[old + 4030..old + 4096].fill(0);
        after[old + 4030..old + 4034].copy_from_slice(&((new << 8) | 1).to_le_bytes());
        after[next + 4030..next + 4096].fill(0);
        after[next + 4030..next + 4034].copy_from_slice(&((new << 8) | 2).to_le_bytes());
        let actual_after =
            fs::read(outputs.join(format!("h{header}-mixed-events-growth.accdb"))).unwrap();
        assert_eq!(actual_after, after);
        assert_eq!(actual_after.len(), actual_before.len());
        read_view(&actual_after, definition, maps, true);
        memberships(&actual_after, maps, (old / 4096) as u32, new);
        assert_eq!(fs::read(path).unwrap(), alternate);
        views += 2;
    }
    assert_eq!(views, 4);
}
