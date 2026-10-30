use super::{base, support};
use std::io::Cursor;
use tablecraft_ace::{InspectError, NativeImage, PayloadPresence, TableExtra, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[allow(dead_code)]
#[path = "../../../../crates/engines/tablecraft-ace/tests/object_payloads/fixture.rs"]
pub(super) mod payload_fixture;

pub(super) fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 16_384, 100_000_000)
}

pub(super) fn source(bytes: &[u8]) -> NativeImage {
    let scope = CatalogScope::new(7);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap();
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
        ["Id", "Score"]
    );
    assert_eq!(
        table.rows(),
        [
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(30)]
        ]
    );
    assert_eq!(table.schema().object_id(), scope.object_id(10));
    let payloads = image
        .inspect_object_payloads(scope.object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(payloads.body(), PayloadPresence::Absent);
    assert_eq!(payloads.module(), PayloadPresence::Absent);
    assert_eq!(payloads.extra(), PayloadPresence::Present);
    assert!(image.as_bytes() == bytes);
    image
}

pub(super) fn extra(image: &NativeImage) -> Result<TableExtra, InspectError> {
    image.inspect_table_extra(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
}
