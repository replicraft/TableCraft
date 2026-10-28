//! Literal GUID segments and complete tuple NULL facts, independent of codecs.
use super::*;
use crate::{NativeImage, table::mutation::insert::tests::multiple_indexes::model};
use tablecraft_core::{limits::ResourceBudget, values::Guid};
use tablecraft_model::CatalogScope;

fn with_reader(run: impl FnOnce(&mut Reader<'_, Cursor<&[u8]>>)) {
    let source = model::source(6, false, false);
    let mut allowance = ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000);
    let image = NativeImage::capture(
        &mut Cursor::new(&source.before),
        CatalogScope::new(1),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    run(&mut reader);
}

fn tuple(required: bool, ignored: bool) -> Profile {
    Profile {
        components: vec![
            Component {
                column: 0,
                kind: Kind::Guid { descending: true },
            },
            Component {
                column: 1,
                kind: Kind::Numeric(numeric::Profile {
                    kind: ScalarType::Int32,
                    width: 4,
                    descending: false,
                    required: false,
                }),
            },
        ],
        required,
        unique: true,
        ignore_nulls: ignored,
    }
}

#[test]
fn guid_literal_segments_keep_descending_continuation_and_reject_corrupt_tags() {
    with_reader(|reader| {
        let value = Value::Guid(Guid::from_bytes([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ]));
        for (descending, expected) in [
            (
                false,
                vec![
                    127, 0, 1, 2, 3, 4, 5, 6, 7, 9, 8, 9, 10, 11, 12, 13, 14, 15, 8,
                ],
            ),
            (
                true,
                vec![
                    128, 255, 254, 253, 252, 251, 250, 249, 248, 9, 247, 246, 245, 244, 243, 242,
                    241, 240, 247,
                ],
            ),
        ] {
            let encoded = guid::encode(&value, descending, 0x01020304, reader).unwrap();
            assert_eq!(&encoded[..19], expected);
            assert_eq!(&encoded[19..], &[1, 2, 3, 4]);
            assert!(matches!(encoded, Wire::Owned(_)));
            assert_eq!(guid::validate(&encoded, descending).unwrap(), (false, 19));
            for tag in [9, 18] {
                let mut bad = encoded.to_vec();
                bad[tag] ^= 1;
                assert!(matches!(
                    guid::validate(&bad, descending),
                    Err(EditError::Preservation)
                ));
            }
            let empty = guid::encode(&Value::Null, descending, 1, reader).unwrap();
            assert_eq!(&empty[..], &[if descending { 255 } else { 0 }, 0, 0, 0, 1]);
            assert!(matches!(
                guid::encode(&Value::Binary(vec![0; 16]), descending, 0, reader),
                Err(EditError::Preservation)
            ));
        }
    });
}

#[test]
fn tuple_nulls_distinguish_all_from_partial_and_required_checks_every_component() {
    with_reader(|reader| {
        let ignored = tuple(false, true);
        let all = ignored
            .request(&[Value::Null, Value::Null], 1, reader)
            .unwrap();
        assert_eq!(&all[..], &[255, 0, 0, 0, 0, 1]);
        let facts = ignored.value(&all).unwrap();
        assert!(facts.all && facts.any && ignored.ignore_nulls);

        let partial = ignored
            .request(&[Value::Null, Value::Int32(7)], 2, reader)
            .unwrap();
        assert_eq!(&partial[..], &[255, 127, 128, 0, 0, 7, 0, 0, 0, 2]);
        let facts = ignored.value(&partial).unwrap();
        assert!(!facts.all && facts.any && ignored.unique);
        assert!(matches!(partial, Wire::Inline { .. }));
        assert!(matches!(
            tuple(true, false).value(&partial),
            Err(EditError::Preservation)
        ));

        let value = Value::Guid(Guid::from_bytes([0; 16]));
        let complete = ignored
            .request(&[value, Value::Int32(7)], 3, reader)
            .unwrap();
        let facts = ignored.value(&complete).unwrap();
        assert!(!facts.all && !facts.any);
        assert!(matches!(complete, Wire::Owned(_)));
        let child = complete.child_paid(99, reader).unwrap();
        assert_eq!(&child[child.len() - 4..], &99_u32.to_be_bytes());
        assert_eq!(&child[..complete.len()], &complete[..]);
    });
}

#[test]
fn owned_wire_allocations_are_charged_and_no_unpaid_wide_constructor_exists() {
    with_reader(|reader| {
        let wide = [127; 23];
        assert!(matches!(Wire::read(&wide), Err(EditError::Preservation)));
        let original = Wire::read_paid(&wide, reader).unwrap();
        let copy = original.copy_paid(reader).unwrap();
        assert_eq!(copy, original);
        let mut empty = ResourceBudget::new(0, 1_000_000, 100_000_000);
        std::mem::swap(reader.budget, &mut empty);
        assert!(matches!(
            original.copy_paid(reader),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    });
}
