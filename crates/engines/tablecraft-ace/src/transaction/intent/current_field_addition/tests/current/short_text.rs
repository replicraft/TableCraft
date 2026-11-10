//! Independent declaration bytes and cold recovery for populated Short Text ADD.
use super::*;
use crate::FieldAdditionTarget;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_text;

fn wire(before: &[u8], width: u16, compressed: bool) -> Vec<u8> {
    let mut bytes = literal(before, "Table1", "Notes");
    bytes[..8].copy_from_slice(b"TCAC0009");
    bytes[12..14].copy_from_slice(&9_u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&2_u16.to_le_bytes());
    bytes[16..18].copy_from_slice(&width.to_le_bytes());
    bytes[18..20].copy_from_slice(&u16::from(compressed).to_le_bytes());
    bytes
}

fn plan(before: &[u8], width: usize, compressed: bool) -> RecoverableCurrentFieldAdditionPlan {
    image(before, SCOPE)
        .plan_recoverable_current_short_text_field_addition(
            "Table1",
            "Notes",
            width,
            compressed,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn declarations_requalify_from_fresh_scope_and_keep_number_language_closed() {
    for (width, compressed) in [(1, false), (37, true), (255, true)] {
        let [before, _] = populated_text::pair("Notes", &[Some(42), None, Some(-7)]);
        let after = populated_text::append_text_declaration(&before, "Notes", width, compressed);
        let reviewed = plan(&before, usize::from(width), compressed);
        assert_eq!(
            reviewed.target(),
            FieldAdditionTarget::ShortText {
                max_utf16_units: usize::from(width),
                unicode_compression: compressed,
            }
        );
        let encoded = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(encoded, wire(&before, width, compressed));
        assert!(matches!(
            Intent::decode(&encoded, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
        assert!(matches!(
            Intent::decode_short_text(&literal(&before, "Table1", "Notes"), &mut budget(), &|| {
                false
            }),
            Err(Error::InvalidRecord)
        ));
        let decoded = Intent::decode_short_text(&encoded, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), encoded);
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
}

#[test]
fn malformed_declarations_refuse_before_owned_names_and_old_decoders_do_not_retry() {
    let [before, _] = populated_text::pair("Notes", &[Some(42)]);
    for (offset, replacement) in [
        (0, 0),
        (8, 0),
        (12, 8),
        (14, 1),
        (16, 0),
        (17, 1),
        (18, 2),
        (19, 1),
        (60, 0),
        (62, 0),
    ] {
        let mut invalid = wire(&before, 255, true);
        invalid[offset] = replacement;
        assert!(
            matches!(
                Intent::decode_short_text(
                    &invalid,
                    &mut ResourceBudget::new(0, 0, 10_000),
                    &|| false,
                ),
                Err(Error::InvalidRecord)
            ),
            "offset {offset}"
        );
    }
}

#[test]
fn cold_pending_and_committed_recovery_require_exact_short_text_candidate() {
    let [before, after] = populated_text::pair("Notes", &[Some(42), None, Some(-7)]);
    for committed in [false, true] {
        // NO_CLONE: Recovery mutates independent storage while BEFORE remains its complete byte oracle.
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&before, 255, true)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            target.bytes.get_mut()[4 * 4096..5 * 4096].copy_from_slice(&after[4 * 4096..5 * 4096]);
        }
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let intent =
            Intent::decode_short_text(&wire(&before, 255, true), &mut budget(), &|| false).unwrap();
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
        // NO_CLONE: Each forgery changes one byte without changing the independent AFTER oracle.
        let mut forged = after.clone();
        forged[offset] ^= 1;
        // NO_CLONE: Journal preparation requires owned mutable storage; BEFORE remains the refusal oracle.
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
        let intent =
            Intent::decode_short_text(&wire(&before, 255, true), &mut budget(), &|| false).unwrap();
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
