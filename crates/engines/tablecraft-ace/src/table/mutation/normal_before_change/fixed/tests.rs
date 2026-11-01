//! Original metadata must preserve previously working indexed services.
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, NativeImage, WriteBlocker, read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::{
    canonical_indexed_before_change as ordinary, canonical_indexed_mixed_before_change as mixed,
    canonical_mixed_before_change as original,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn preserves_original_service(bytes: &[u8]) {
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(111),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        !image
            .select_native_before_change_fixed_table("Items", &mut budget(), &|| false)
            .unwrap()
    );
    // This is readonly complete compilation of the old context, not a producer
    // invocation or an expected image reconstructed by that producer.
    image
        .check_native_before_change_int32_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn composite_indexes_select_the_existing_service_before_preserved_capture() {
    for header in [5, 6] {
        let mixed = mixed::source(header, mixed::Recipe::CounterCompositeTie);
        preserves_original_service(&mixed.before);
        let ordinary = ordinary::source(header, ordinary::Recipe::Composite);
        preserves_original_service(&ordinary.before);
    }
}

#[test]
fn readable_complex_fk_marker_does_not_admit_the_new_fixed_writer() {
    for header in [5, 6] {
        let original = original::source(header, original::Recipe::FixedSequential);
        let baseline = original.before;
        let mut bytes = baseline.clone();
        let mut rows = original.rows;
        let descriptor = 7 * 4096 + 63 + 4 * 25;
        // Explicit independently authored descriptor derivative: reuse the
        // unreferenced eight-byte Int64 slot as a four-byte FK-marked scalar,
        // retaining the upper four physical bytes and all actual row bytes.
        assert_eq!(&bytes[7 * 4096 + 51..7 * 4096 + 55], &[0; 4]);
        assert_eq!(bytes[descriptor], 19);
        assert_eq!(&bytes[descriptor + 5..descriptor + 7], &[4, 0]);
        assert_eq!(&bytes[descriptor + 15..descriptor + 17], &[3, 0]);
        assert_eq!(&bytes[descriptor + 21..descriptor + 25], &[7, 0, 8, 0]);
        bytes[descriptor] = 4;
        bytes[descriptor + 16] = 0x08;
        bytes[descriptor + 23..descriptor + 25].copy_from_slice(&4_u16.to_le_bytes());
        for row in &mut rows {
            assert_eq!(row[4], Value::Int64(0));
            row[4] = Value::Int32(0);
        }
        for (at, (before, after)) in baseline.iter().zip(&bytes).enumerate() {
            if ![
                descriptor,
                descriptor + 16,
                descriptor + 23,
                descriptor + 24,
            ]
            .contains(&at)
            {
                assert_eq!(before, after, "unrelated marker-control byte {at}");
            }
        }
        let scope = CatalogScope::new(112);
        let image = NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false)
            .unwrap();
        let read = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(read.columns().len(), 10);
        assert_eq!(read.rows(), rows);
        let marked = &read.schema().columns()[4];
        assert_eq!(marked.native_type(), 4);
        assert_eq!(marked.extended_flags(), 0x08);
        assert_eq!(marked.storage_length(), 4);
        assert_eq!(marked.scalar_type(), Some(ScalarType::Int32));
        // This proves the raw scalar exception and complete scalar decoding,
        // not a healthy complex parent/child layout or native complex behavior.
        assert!(
            !image
                .select_native_before_change_fixed_table("Items", &mut budget(), &|| false)
                .unwrap()
        );
        assert!(matches!(
            image.check_native_before_change_fixed_table("Items", &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        assert_eq!(image.as_bytes(), bytes);

        let updates = [FixedScalarUpdate::new(
            0,
            Value::Boolean(false),
            Value::Boolean(true),
        )];
        let groups = [FixedColumnUpdates::new("Items", "Flag", &updates)];
        assert!(matches!(
            image.plan_before_change_fixed_updates("Items", &groups, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
}
