//! Independent GUID UPDATE AFTER reads; no planner, writer or journal calls.
use super::{SCOPE, budget};
use crate::{
    GuidIndexSelection, IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter,
    inspect_table_schema, read_table, read_table_guid_index, read_table_index,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_indexed_guid_update::raw::{
    ImageSpec, PAGE, RECORD_BYTES, entries, geometry, guid, indexed_records, leaf, load_views,
    new_guid, values,
};

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn inspect(case: &ImageSpec, raw: &[u8], references: &[(u32, Vec<u8>)]) {
    let rows = values(case.kind);
    let (definition, _, roots) = geometry(case.header);
    let primary = entries(&rows, references, false, false);
    leaf(raw, definition, roots[0], &primary);
    for offset in [67, 79] {
        let at = definition * PAGE + offset;
        assert_eq!(&raw[at..at + 4], &158_u32.to_le_bytes());
    }

    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(schema.definition_page() as usize, definition);
    assert_eq!(schema.columns().len(), 3);
    for (position, field) in schema.columns().iter().enumerate() {
        assert_eq!(field.number(), position);
        assert_eq!(field.name(), ["ID", "Group", "Score"][position]);
        assert_eq!(field.native_type(), [4, 15, 4][position]);
        assert_eq!(field.storage_length(), [4, 16, 4][position]);
        assert!(field.fixed());
        assert_eq!((field.flags(), field.extended_flags()), (3, 0));
    }
    let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(read.rows(), rows);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 158);
    assert_eq!(checked.indexes().len(), 4);
    let mut maps = [None; 2];
    let mut aliases = [0; 2];
    let secondary = entries(&rows, references, true, case.descending);
    for checked in checked.indexes() {
        assert_eq!(checked.status(), IndexCheckStatus::Checked);
        let index = checked.index();
        let position = match index.name() {
            "PrimaryID" | "PrimaryAlias" => 0,
            "GroupOrder" | "GroupAlias" => 1,
            _ => panic!("outside the four frozen aliases"),
        };
        aliases[position] += 1;
        assert_eq!(index.data_number(), position as u32);
        assert_eq!(index.root_page() as usize, roots[position]);
        assert_eq!(index.columns().len(), 1);
        assert_eq!(index.columns()[0].number(), position);
        assert_eq!(
            index.columns()[0].descending(),
            position == 1 && case.descending
        );
        if let Some(prior) = maps[position] {
            assert_eq!(index.usage_map(), prior);
        } else {
            maps[position] = Some(index.usage_map());
        }
        let flags = index.constraints().unwrap();
        assert_eq!(flags.primary_key(), index.name() == "PrimaryID");
        assert_eq!(flags.required(), position == 0);
        assert_eq!(flags.unique(), position == 0);
        assert!(!flags.ignore_nulls());
        let order = if position == 0 { &primary } else { &secondary };
        let observed = if position == 0 {
            read_table_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                index.name(),
                &mut budget(),
                &|| false,
            )
        } else {
            read_table_guid_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                index.name(),
                GuidIndexSelection::All,
                &mut budget(),
                &|| false,
            )
        }
        .unwrap();
        let ordered_rows: Vec<_> = order
            .iter()
            .map(|(_, ordinal)| rows[*ordinal].clone())
            .collect();
        assert_eq!(observed.rows(), ordered_rows);
        let history = inspect_table_index_counter(
            &mut image.reader(),
            SCOPE,
            "Items",
            index.name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(history.value(), 158);

        if position == 1 {
            let nulls = read_table_guid_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                index.name(),
                GuidIndexSelection::Null,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let expected_nulls: Vec<_> = order
                .iter()
                .filter(|(_, ordinal)| rows[*ordinal][1] == Value::Null)
                .map(|(_, ordinal)| rows[*ordinal].clone())
                .collect();
            assert_eq!(nulls.rows(), expected_nulls);
            for selected in [guid(1), guid(156), new_guid()] {
                let equal = read_table_guid_index(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    index.name(),
                    GuidIndexSelection::Equal(selected),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let expected_equal: Vec<_> = order
                    .iter()
                    .filter(|(_, ordinal)| rows[*ordinal][1] == Value::Guid(selected))
                    .map(|(_, ordinal)| rows[*ordinal].clone())
                    .collect();
                assert_eq!(equal.rows(), expected_equal);
            }
        }
    }
    assert_eq!(aliases, [2, 2]);
    assert_ne!(maps[0], maps[1]);
    NoRequiredTablePrograms::capture_updates(&image, &schema, &mut budget(), &|| false).unwrap();
    let source = normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), rows);
    assert_eq!(source.records.len(), 158);
    for (record, (reference, _)) in source.records.iter().zip(references) {
        assert_eq!(record.reference, *reference);
        let page = usize::try_from(reference >> 8).unwrap();
        let slot = usize::try_from(reference & 255).unwrap();
        let start = page * PAGE + PAGE - RECORD_BYTES * (slot + 1);
        assert_eq!(record.data, raw[start..start + RECORD_BYTES]);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "four original and24 independent GUID UPDATE AFTER reads; no mutation"]
fn independently_authored_indexed_guid_updates_keep_complete_source_and_membership() {
    let views = load_views(|raw, pin| assert_eq!(digest(raw), pin));
    let original_records: Vec<_> = views
        .iter()
        .enumerate()
        .filter(|(_, view)| view.spec.kind == 0)
        .map(|(position, view)| (position, indexed_records(&view.raw, view.spec.header)))
        .collect();
    assert_eq!(original_records.len(), 4);
    for view in &views {
        let (_, references) = original_records
            .iter()
            .find(|(position, _)| {
                let original = &views[*position].spec;
                original.header == view.spec.header && original.descending == view.spec.descending
            })
            .unwrap();
        inspect(&view.spec, &view.raw, references);
    }
    assert_eq!(views.len(), 28);
}
