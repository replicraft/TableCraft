//! Reader-valid alternate encodings cannot replace exact request wire authority.
use super::super::super::{admission, encode::Encoded, stage};
use crate as tablecraft_ace;
use crate::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate, check_table,
    check_table_constraints, check_table_long_values, read_table,
    test_support::{fixture, support},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[allow(dead_code)]
#[path = "../../../../../../tests/ace16_long_value_updates_contract/model.rs"]
mod model;

#[test]
fn independently_valid_decoding_does_not_authorize_the_wrong_encoding_policy() {
    for (compressed, value, forged) in [
        (true, "ééé".to_owned(), vec![0xe9, 0, 0xe9, 0, 0xe9, 0]),
        (false, "ABC".to_owned(), vec![0xff, 0xfe, 0x41, 0x42, 0x43]),
        (
            true,
            "A".repeat(1025),
            [vec![0xff, 0xfe], vec![0x41; 1025]].concat(),
        ),
    ] {
        let mut before = model::source(6, 1);
        before[10 * 4096 + 104] = u8::from(compressed);
        let image = model::capture(&before);
        let prepared =
            admission::prepare(&image, "Items", "Memo", &mut model::budget(), &|| false).unwrap();
        let requests = [SlotScalarUpdate::new(
            0,
            Value::Text("A".into()),
            Value::Text(value.clone()),
        )];
        let mut staged = stage::build(&image, &prepared, &requests, &mut model::budget(), &|| {
            false
        })
        .unwrap();
        let field = &prepared.structural.schema().columns()[prepared.column];
        super::prove(field, &staged.changes[0], &mut model::budget(), &|| false).unwrap();
        assert_eq!(
            crate::format::text::decode_long_with_limits(
                &forged,
                65536,
                65536,
                &mut model::budget(),
                &|| false
            )
            .unwrap(),
            value
        );
        staged.changes[0].encoded = Encoded::Present(forged);
        assert_eq!(
            super::prove(field, &staged.changes[0], &mut model::budget(), &|| false).err(),
            Some(EditError::Preservation)
        );
    }
}
