//! Independent wire and retained-journal proof for current Number additions.
use super::{
    record::{
        CurrentFieldAdditionIntentError as Error, NativeCurrentFieldAdditionIntent as Intent,
    },
    recovery::qualify_native_current_field_addition_recovery,
};
use crate::{NativeImage, RecoverableCurrentFieldAdditionPlan};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, inspect_v2};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::populated,
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(1006);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

fn image(bytes: &[u8], scope: CatalogScope) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8], table: &str) -> RecoverableCurrentFieldAdditionPlan {
    image(before, SCOPE)
        .plan_recoverable_current_number_field_addition(table, "Bonus", &mut budget(), &|| false)
        .unwrap()
}

fn literal(before: &[u8], table: &str, column: &str) -> Vec<u8> {
    let mut bytes = b"TCAC0008".to_vec();
    bytes.extend(((64 + table.len() + column.len()) as u32).to_le_bytes());
    bytes.extend(8_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend([0; 4]);
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    bytes.extend(hash.finalize());
    bytes.extend((table.len() as u16).to_le_bytes());
    bytes.extend((column.len() as u16).to_le_bytes());
    bytes.extend(table.as_bytes());
    bytes.extend(column.as_bytes());
    bytes
}

#[test]
fn current_number_wire_and_fresh_scoped_requalification_match_independent_images() {
    let [before, after] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let reviewed = plan(&before, "Table1");
    let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(wire, literal(&before, "Table1", "Bonus"));
    assert_eq!(
        Intent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .encode(&mut budget(), &|| false)
            .unwrap(),
        wire
    );
    let fresh = reviewed
        .requalify_with_budgets(
            image(&before, CatalogScope::new(1007)),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(fresh.before_bytes(), before);
    assert_eq!(fresh.after_bytes(), after);
}

#[test]
fn header_target_and_borrowed_name_refusals_precede_owned_name_allocation() {
    let [before, _] = populated::pair("Bonus", &[Some(42)]);
    let good = literal(&before, "Table1", "Bonus");
    for offset in [0, 8, 12, 14, 16, 18, 60, 62] {
        // NO_CLONE: Each corruption needs an independent buffer while the original wire remains intact.
        let mut invalid = good.clone();
        invalid[offset] ^= 1;
        assert!(matches!(
            Intent::decode(&invalid, &mut ResourceBudget::new(0, 100, 10_000), &|| {
                false
            }),
            Err(Error::InvalidRecord)
        ));
    }
    for column in ["", "雪", "bad\0name"] {
        assert!(matches!(
            Intent::decode(
                &literal(&before, "Table1", column),
                &mut ResourceBudget::new(0, 100, 10_000),
                &|| false
            ),
            Err(Error::InvalidRecord)
        ));
    }
    assert!(
        crate::NativeFieldAdditionAfterOffHistoryIntent::decode(&good, &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn unicode_wire_names_do_not_grant_unqualified_native_addition_authority() {
    let [mut before, _] = populated::pair("Bonus", &[Some(42), None]);
    let old = "Table1"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let table = "表表表表表表";
    let new = table
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(old.len(), new.len());
    let positions = before
        .windows(old.len())
        .enumerate()
        .filter_map(|(at, bytes)| (bytes == old).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    before[positions[0]..positions[0] + new.len()].copy_from_slice(&new);
    // This synthetic catalog rename tests the persisted request boundary only.
    // It does not qualify Unicode native metadata or collision semantics.
    let wire = literal(&before, table, "Bonus");
    let decoded = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), table);
    assert_eq!(decoded.column(), "Bonus");
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    assert!(matches!(
        decoded.reconstruct(image(&before, SCOPE), &mut budget(), &|| false),
        Err(Error::Field(crate::CurrentFieldAdditionError::Source))
    ));
}

#[test]
fn cold_current_number_recovery_checks_full_images_and_retains_actual_records() {
    let [before, after] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let wire = literal(&before, "Table1", "Bonus");
    for committed in [false, true] {
        // NO_CLONE: Recovery mutates storage; retain the independent original image as its byte oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&before, "Table1")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            target.bytes.get_mut()[4 * 4096..5 * 4096].copy_from_slice(&after[4 * 4096..5 * 4096]);
        }
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
        qualify_native_current_field_addition_recovery(
            checked,
            intent,
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .recover(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(
            target.bytes.get_ref(),
            if committed { &after } else { &before }
        );
    }

    for offset in [
        7 * 4096 + 20,
        8 * 4096 + 4090,
        9 * 4096 + 482,
        before.len() - 1,
    ] {
        // NO_CLONE: Alter one byte per case without changing the independently authored AFTER oracle.
        let mut forged = after.clone();
        forged[offset] ^= 1;
        // NO_CLONE: Journal preparation owns mutable storage while the original remains the refusal oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_expected(
                &mut target,
                &mut journal,
                &before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_current_field_addition_recovery(
                checked,
                intent,
                SCOPE,
                &mut budget(),
                &|| false
            ),
            Err(Error::ImageMismatch)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}

mod short_text;
