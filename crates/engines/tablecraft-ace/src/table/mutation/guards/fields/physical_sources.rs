//! Independent full images before the explicit physical SetField consumer.
mod api;
pub(super) mod model;

use crate::{
    FixedScalarUpdate, IndexCheckStatus, InspectError, MacroProgramCoverage, MacroProgramNodeKind,
    MacroProgramProfile, NativeImage, check_table, check_table_constraints,
    format::{
        allocation::{self, MapRole, RepresentedMap},
        reader::Reader,
    },
    read_table, read_table_index,
    table::mutation::{gates, page_update::roots, variable_split::metadata::Metadata},
};
use std::{collections::BTreeMap, io::Cursor};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 500_000_000)
}

pub(super) fn values(
    row: &crate::table::mutation::fixed::batch::split_tests::model::Row,
) -> Vec<Value> {
    vec![
        Value::Int32(row.id),
        row.left.map_or(Value::Null, Value::Int32),
        row.right.map_or(Value::Null, Value::Int32),
    ]
}

// Independent literal height-zero/prefix-zero fixture decoder. Production
// check_table separately supplies its qualified ignored-membership traversal.
pub(super) fn raw_wires(page: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(page.len(), 4096);
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(&page[4..8], &10_u32.to_le_bytes());
    assert_eq!(&page[12..20], &[0; 8]);
    assert_eq!(&page[24..26], &[0, 0]);
    let free = usize::from(u16::from_le_bytes(page[2..4].try_into().unwrap()));
    assert!(free <= 3616);
    let active = 3616 - free;
    let bitmap = &page[27..480];
    assert_eq!(bitmap[0] & 1, 0);

    let mut cursor = 0;
    let mut wires = Vec::new();
    for end in 1..=active {
        if bitmap[end / 8] & (1 << (end % 8)) != 0 {
            wires.push(page[480 + cursor..480 + end].to_vec());
            cursor = end;
        }
    }
    assert_eq!(cursor, active);
    for end in active + 1..bitmap.len() * 8 {
        assert_eq!(bitmap[end / 8] & (1 << (end % 8)), 0);
    }
    wires
}

fn qualify(case: &model::Case, descending: bool, expected: bool) {
    let (bytes, rows) = if expected {
        (&case.expected, &case.final_rows)
    } else {
        (&case.before, &case.original)
    };
    let scope = CatalogScope::new(7);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 500);
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    gates::qualified(
        &check_table_constraints(&mut image.reader(), scope, "Items", &mut budget(), &|| {
            false
        })
        .unwrap(),
    )
    .unwrap();
    let table = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(
        table
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["ID", "Group", "Score"]
    );
    assert_eq!(table.rows(), rows.iter().map(values).collect::<Vec<_>>());

    let by_reference = rows
        .iter()
        .map(|row| (row.reference, row))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(by_reference.len(), 500);
    for (physical, names) in [["LeftOrder", "LeftAlias"], ["PairOrder", "PairAlias"]]
        .into_iter()
        .enumerate()
    {
        let wires = model::entries(rows, physical, descending);
        let page = (13 + physical) * model::PAGE;
        assert_eq!(raw_wires(&bytes[page..page + model::PAGE]), wires);
        let ordered = wires
            .into_iter()
            .map(|wire| {
                let reference = u32::from_be_bytes(wire[wire.len() - 4..].try_into().unwrap());
                values(by_reference[&reference])
            })
            .collect::<Vec<_>>();
        for name in names {
            let definition = checked
                .indexes()
                .iter()
                .find(|index| index.index().name() == name)
                .unwrap();
            assert_eq!(definition.index().data_number(), physical as u32);
            assert_eq!(definition.index().root_page(), 13 + physical as u32);
            assert_eq!(
                definition.index().constraints().unwrap().ignore_nulls(),
                physical == 1
            );
            let read = read_table_index(
                &mut image.reader(),
                scope,
                "Items",
                name,
                &mut budget(),
                &|| false,
            );
            if physical == 0 {
                assert_eq!(read.unwrap().rows(), ordered);
            } else {
                // This API promises complete-table coverage, not the 200/500
                // physical subset. Its existing refusal remains explicit.
                assert_eq!(read.err(), Some(InspectError::Unsupported));
                let eligible = table
                    .rows()
                    .iter()
                    .filter(|row| row[1] != Value::Null || row[2] != Value::Null)
                    .count();
                assert_eq!(eligible, 200);
                assert_eq!(ordered.len(), eligible);
            }
        }
    }
    for (physical, history) in [700_u32, 800].into_iter().enumerate() {
        let offset = 10 * 4096 + 67 + physical * 12;
        assert_eq!(&bytes[offset..offset + 4], &history.to_le_bytes());
    }
    let extra = image
        .inspect_table_extra(scope.object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(extra.raw(), Some(case.extra.as_slice()));
    let properties = extra.groups()[0].properties().unwrap();
    assert_eq!(properties.len(), 1);
    assert_eq!(properties[0].text(), Some(case.xml.as_str()));
    let program = properties[0]
        .inspect_macro_program(
            MacroProgramProfile::BeforeChangeFields,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        program.coverage(),
        MacroProgramCoverage::RecognizedDesktopSubset
    );
    assert_eq!(
        program
            .nodes()
            .iter()
            .filter(|node| matches!(node.kind(), MacroProgramNodeKind::SetField { .. }))
            .count(),
        2
    );

    // This is original/expected read-only physical ownership qualification, not
    // a mutation gate override, reservation, candidate or evaluator invocation.
    let mut resources = budget();
    let metadata = Metadata::capture_for_fitted_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1, 2],
        &mut resources,
        &|| false,
    )
    .unwrap();
    metadata
        .require_source(&image, checked.schema().definition_page())
        .unwrap();
    let roots = roots::capture(
        &image,
        checked.schema(),
        &[] as &[FixedScalarUpdate],
        &mut resources,
        &|| false,
    )
    .unwrap();
    for &page in &roots.owned {
        metadata
            .require_data_page(page, &mut resources, &|| false)
            .unwrap();
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut resources,
        cancelled: &|| false,
        reads: 0,
    };
    let global = RepresentedMap::read(
        &mut reader,
        allocation::GLOBAL_FREE_REFERENCE,
        MapRole::GlobalFree,
    )
    .unwrap();
    global
        .require_allocated_storage(&mut reader, &global)
        .unwrap();
    for page in roots.owned {
        global.require_allocated(&mut reader, page).unwrap();
    }
    for index in checked.indexes() {
        let owned =
            RepresentedMap::read(&mut reader, index.index().usage_map(), MapRole::Owned).unwrap();
        owned
            .require_allocated_storage(&mut reader, &global)
            .unwrap();
        for page in allocation::pages(&mut reader, index.index().usage_map()).unwrap() {
            global.require_allocated(&mut reader, page).unwrap();
        }
    }
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_before_change_setfield_complete_physical_images_before_consumer() {
    let mut pairs = 0;
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                let case = model::source(code, descending, unique, false);
                assert!(case.changed);
                qualify(&case, descending, false);
                qualify(&case, descending, true);
                assert_eq!(case.before.len(), case.expected.len());
                assert_eq!(
                    &case.before[4 * model::PAGE..5 * model::PAGE],
                    &case.expected[4 * model::PAGE..5 * model::PAGE]
                );
                pairs += 1;
            }
        }
        let rejected = model::source(code, false, false, true);
        assert!(!rejected.changed);
        assert_eq!(rejected.before, rejected.expected);
        qualify(&rejected, false, false);
        qualify(&rejected, false, true);
        pairs += 1;
    }
    assert_eq!(pairs, 10);
}
