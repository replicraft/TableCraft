use super::budget;
use crate::{
    FixedScalarUpdate, MacroProgramCoverage, MacroProgramNodeKind, MacroProgramProfile,
    NativeImage, PayloadPresence, RowDelete, RowInsert, TableExtraCoverage, read_table,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

mod api;
pub(crate) mod cases;
pub(crate) mod model;
mod ordinal_source;

#[test]
fn qualify_native_before_event_guard_sources_and_literal_traces_before_execution() {
    let mut outcomes = [0_usize; 7];
    for case in cases::CASES {
        for code in [5, 6] {
            let (bytes, xml, bag) = model::source(code, case.event, case.body);
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
                ["Id", "Score"],
                "{} / header {code}",
                case.name
            );
            let expected = model::ORIGINAL_ROWS
                .iter()
                .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            assert_eq!(table.rows(), expected, "{} / header {code}", case.name);

            let object = scope.object_id(10);
            assert_eq!(table.schema().object_id(), object);
            let payloads = image
                .inspect_object_payloads(object, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(payloads.body(), PayloadPresence::Absent);
            assert_eq!(payloads.module(), PayloadPresence::Absent);
            assert_eq!(payloads.extra(), PayloadPresence::Present);
            let extra = image
                .inspect_table_extra(object, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(extra.object_id(), object);
            assert_eq!(extra.native_id(), 10);
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 2));
            assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
            assert_eq!(extra.presence(), PayloadPresence::Present);
            assert_eq!(extra.raw(), Some(bag.as_slice()));
            assert_eq!(extra.names(), ["StoredGuard"]);
            let properties = extra.groups()[0].properties().unwrap();
            assert_eq!(properties.len(), 1);
            assert_eq!(properties[0].name(), "StoredGuard");
            assert_eq!(
                (properties[0].native_type(), properties[0].flags()),
                (12, 1)
            );
            assert_eq!(properties[0].text(), Some(xml.as_str()));
            assert_eq!(
                properties[0].raw(),
                xml.encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
            );
            let program = properties[0]
                .inspect_macro_program(
                    MacroProgramProfile::DocumentedDesktop,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(program.xml_source(), xml);
            assert_eq!(program.envelopes().definitions().len(), 1);
            assert_eq!(
                program.envelopes().definitions()[0].event(),
                Some(case.event)
            );
            let mut nodes = [0_usize; 3];
            for node in program.nodes() {
                match node.kind() {
                    MacroProgramNodeKind::Condition { .. } => nodes[0] += 1,
                    MacroProgramNodeKind::RaiseError { .. } => nodes[1] += 1,
                    MacroProgramNodeKind::Opaque { .. } => nodes[2] += 1,
                    _ => {}
                }
            }
            assert_eq!(
                nodes,
                [case.conditions, case.raises, case.opaque],
                "{}",
                case.name
            );
            assert_eq!(
                program.coverage(),
                if case.opaque == 0 {
                    MacroProgramCoverage::RecognizedDesktopSubset
                } else {
                    MacroProgramCoverage::ContainsOpaqueStructure
                }
            );

            // These are the actual existing mutation request carriers, not an
            // invented producer-expanded row cache. No evaluator is called.
            match case.intent {
                cases::Intent::Update {
                    row,
                    expected,
                    value,
                } => {
                    assert_eq!(model::ORIGINAL_ROWS[row][1], expected);
                    let update =
                        FixedScalarUpdate::new(row, Value::Int32(expected), Value::Int32(value));
                    assert_eq!(update.row(), row);
                    assert_eq!(update.expected(), &table.rows()[row][1]);
                    assert_eq!(update.value(), &Value::Int32(value));
                }
                cases::Intent::Insert { id, score } => {
                    let insert = RowInsert::new(vec![Value::Int32(id), Value::Int32(score)]);
                    assert_eq!(insert.values(), [Value::Int32(id), Value::Int32(score)]);
                }
                cases::Intent::Delete { row } => {
                    let expected = model::original_record(row);
                    let delete = RowDelete::new(row, expected.clone());
                    assert_eq!(delete.row(), row);
                    assert_eq!(delete.expected_bytes(), expected);
                    let page = &bytes[12 * model::PAGE..13 * model::PAGE];
                    let offset = 14 + row * 2;
                    let start =
                        usize::from(u16::from_le_bytes([page[offset], page[offset + 1]])) & 0x0fff;
                    assert_eq!(&page[start..start + expected.len()], expected);
                }
                cases::Intent::UpdateTwo => {
                    for (row, expected, value) in [(0, 10, 11), (1, 20, 19)] {
                        let update = FixedScalarUpdate::new(
                            row,
                            Value::Int32(expected),
                            Value::Int32(value),
                        );
                        assert_eq!(update.expected(), &table.rows()[row][1]);
                        assert_eq!(update.value(), &Value::Int32(value));
                    }
                }
            }
            assert!(!case.trace.is_empty());
            let outcome = match case.outcome {
                cases::Outcome::Pass => 0,
                cases::Outcome::Raised {
                    number,
                    description,
                } => {
                    assert!(number > 0);
                    if let Some(description) = description {
                        assert!(!description.contains('\0'));
                    }
                    1
                }
                cases::Outcome::UnsupportedContext => 2,
                cases::Outcome::UnsupportedCondition => 3,
                cases::Outcome::UnsupportedStructure => 4,
                cases::Outcome::ExpressionError => 5,
                cases::Outcome::ArgumentType => 6,
            };
            outcomes[outcome] += 1;
            assert_eq!(image.as_bytes(), bytes, "{} source changed", case.name);
        }
    }
    assert_eq!(outcomes, [6, 14, 8, 4, 2, 2, 4]);
}
