//! Literal TCAC13 originals retain policy absence and complete source binding.
mod recovery;

use super::{
    DeclaredFieldAdditionIntentError as Error, NativeDeclaredFieldAdditionIntent as Intent,
};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn literal(before: &[u8], table: &str, column: &str, definition: u32) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = b"TCAC0013".to_vec();
    wire.extend(((80 + table.len() + column.len()) as u32).to_le_bytes());
    wire.extend(13_u16.to_le_bytes());
    wire.extend(0_u16.to_le_bytes());
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend((definition as i32).to_le_bytes());
    wire.extend(definition.to_le_bytes());
    wire.extend(2_u16.to_le_bytes());
    wire.extend(2_u16.to_le_bytes());
    wire.extend(2_u16.to_le_bytes());
    wire.extend([5, 0]);
    wire.extend((table.len() as u16).to_le_bytes());
    wire.extend((column.len() as u16).to_le_bytes());
    wire.extend(0_u32.to_le_bytes());
    wire.extend(table.as_bytes());
    wire.extend(column.as_bytes());
    wire
}

#[test]
fn literal_declared_original_keeps_absent_policies_unicode_table_and_signed_identity() {
    let before = vec![0; 20 * 4096];
    for table in ["Items".to_string(), "界".repeat(64), "🦀".repeat(32)] {
        let mut wire = literal(&before, &table, "Amount", 14);
        wire[56..60].copy_from_slice(&(-37_i32).to_le_bytes());
        let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.table(), table);
        assert_eq!(intent.column(), "Amount");
        assert_eq!(intent.native_table_id(), -37);
        assert_eq!(intent.definition_page(), 14);
        assert_eq!(intent.native_number(), 2);
        assert_eq!(intent.column_id(), 2);
        assert_eq!(intent.column_count(), 2);
        let column = intent.definition().column();
        assert_eq!(
            column.scalar_type(),
            tablecraft_schema::ScalarType::Currency
        );
        assert_eq!(column.required(), None);
        assert_eq!(column.allow_zero_length(), None);
        assert!(column.default_literal().is_none());
        assert_eq!(column.description(), None);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
        intent
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        let mut stale = before.to_vec();
        stale[7000] ^= 1;
        assert!(matches!(
            intent.verify_before(&stale, &mut budget(), &|| false),
            Err(Error::BeforeMismatch)
        ));
    }
}

#[test]
fn declared_original_framing_is_closed_and_old_gallery_decoder_is_not_a_retry() {
    let before = vec![0; 20 * 4096];
    let wire = literal(&before, "Items", "Amount", 14);
    for end in 0..wire.len() {
        assert!(Intent::decode(&wire[..end], &mut budget(), &|| false).is_err());
    }
    for (offset, value) in [
        (0, b'X'),
        (7, b'2'),
        (8, 0),
        (12, 12),
        (14, 1),
        (60, 0),
        (68, 0),
        (70, 4),
        (71, 1),
        (72, 0),
        (74, 0),
        (76, 1),
    ] {
        let mut invalid = wire.to_vec();
        invalid[offset] = value;
        assert!(
            Intent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    let mut extra = wire.to_vec();
    extra.push(0);
    assert!(Intent::decode(&extra, &mut budget(), &|| false).is_err());
    for (table, column) in [("Items", "ſ"), ("Items", "bad.name"), ("", "Amount")] {
        assert!(
            Intent::decode(&literal(&before, table, column, 14), &mut budget(), &|| {
                false
            })
            .is_err()
        );
    }
    assert!(
        crate::NativeRetainedTypedFieldAdditionIntent::decode(&wire, &mut budget(), &|| false)
            .is_err()
    );
    let mut old = wire.to_vec();
    old[7] = b'2';
    old[12..14].copy_from_slice(&12_u16.to_le_bytes());
    assert!(Intent::decode(&old, &mut budget(), &|| false).is_err());
    assert!(Intent::decode(&wire, &mut ResourceBudget::new(0, 0, 0), &|| false).is_err());
    assert!(Intent::decode(&wire, &mut budget(), &|| true).is_err());
}
