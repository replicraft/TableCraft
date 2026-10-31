use super::{budget, guard_sources::model};
use crate::{
    FixedScalarUpdate, MacroProgramCoverage, MacroProgramNodeKind, MacroProgramProfile,
    NativeImage, PayloadPresence, RowInsert, TableExtraCoverage, read_table,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

mod api;
mod cases;

fn row(values: &[i32; 2]) -> Vec<Value> {
    values.iter().copied().map(Value::Int32).collect()
}

#[test]
fn qualify_before_change_setfield_sources_and_final_rows_before_evaluation() {
    let mut admitted = 0;
    for case in cases::CASES {
        for code in [5, 6] {
            let (bytes, xml, bag) = model::source(code, "BeforeChange", case.body);
            let scope = CatalogScope::new(7);
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false)
                    .unwrap_or_else(|error| {
                        panic!("{} / header {code}: capture {error}", case.name)
                    });
            let table = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                false
            })
            .unwrap_or_else(|error| panic!("{} / header {code}: rows {error}", case.name));
            assert_eq!(
                table
                    .columns()
                    .iter()
                    .map(|column| column.name())
                    .collect::<Vec<_>>(),
                ["Id", "Score"]
            );
            assert_eq!(
                table.rows(),
                model::ORIGINAL_ROWS.iter().map(row).collect::<Vec<_>>()
            );

            let extra = image
                .inspect_table_extra(scope.object_id(10), &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{} / header {code}: Extra {error}", case.name));
            assert_eq!(extra.presence(), PayloadPresence::Present);
            assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
            assert_eq!(extra.raw(), Some(bag.as_slice()));
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 2));
            assert_eq!(extra.names(), ["StoredGuard"]);
            let properties = extra.groups()[0].properties().unwrap();
            assert_eq!(properties.len(), 1);
            let property = &properties[0];
            assert_eq!((property.native_type(), property.flags()), (12, 1));
            assert_eq!(property.name(), "StoredGuard");
            assert_eq!(property.text(), Some(xml.as_str()));
            assert_eq!(
                property.raw(),
                xml.encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
            );

            // The existing profile must stay opaque here. No new evaluator or
            // new structural profile is linked into this SOURCE prerequisite.
            let program = property
                .inspect_macro_program(
                    MacroProgramProfile::DocumentedDesktop,
                    &mut budget(),
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("{} / header {code}: program {error}", case.name));
            assert_eq!(program.xml_source(), xml);
            assert_eq!(program.envelopes().definitions().len(), 1);
            assert_eq!(
                program.envelopes().definitions()[0].event(),
                Some("BeforeChange")
            );
            assert_eq!(
                program.coverage(),
                MacroProgramCoverage::ContainsOpaqueStructure
            );
            assert!(
                program
                    .nodes()
                    .iter()
                    .any(|node| matches!(node.kind(), MacroProgramNodeKind::Opaque { .. }))
            );

            if let Some(original) = case.original {
                assert_eq!(original.len(), case.proposed.len());
                for (ordinal, (before, proposed)) in original.iter().zip(case.proposed).enumerate()
                {
                    assert_eq!(*before, model::ORIGINAL_ROWS[ordinal]);
                    assert_eq!(before[0], proposed[0]);
                    let update = FixedScalarUpdate::new(
                        ordinal,
                        Value::Int32(before[1]),
                        Value::Int32(proposed[1]),
                    );
                    assert_eq!(update.expected(), &table.rows()[ordinal][1]);
                    assert_eq!(update.value(), &Value::Int32(proposed[1]));
                }
            } else {
                for proposed in case.proposed {
                    let insert = RowInsert::new(row(proposed));
                    assert_eq!(insert.values(), row(proposed));
                }
            }
            if let Some(final_rows) = case.final_rows {
                assert_eq!(case.outcome, "pass");
                assert_eq!(final_rows.len(), case.proposed.len());
            } else {
                assert_ne!(case.outcome, "pass");
            }
            assert_eq!(image.as_bytes(), bytes, "{} source changed", case.name);
            admitted += 1;
        }
    }
    assert_eq!(admitted, 36);
}
