//! Direct original-only proof and writable INDEX ownership controls.
use super::proof;
use crate::{
    EditError, IndexCheckStatus, IndexConstraintViolation, InspectError, Int32Update,
    NativeEditPlan, NativeImage, WriteBlocker, check_table,
    table::indexes::mutation::normal_index_audit::Ownership,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_indexed_before_change::{self as model, Case, Recipe};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(97)
}

fn updates(case: &Case) -> Vec<Int32Update> {
    case.updates
        .iter()
        .map(|update| Int32Update::new(update.ordinal, update.expected, update.replacement))
        .collect()
}

fn prove(plan: &NativeEditPlan, updates: &[Int32Update]) -> Result<(), EditError> {
    proof::prove(
        plan,
        scope(),
        "Items",
        "Score",
        updates,
        &mut budget(),
        &|| false,
    )
}

fn observation(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone(),
        after: case.expected.as_ref().unwrap().clone(),
        changed_rows: case.changed_rows,
    }
}

fn pointer(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * 4096 + 14 + 2 * slot;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    page * 4096 + usize::from(word)
}

#[test]
fn indexed_independent_proof_accepts_all_literal_images_and_rejects_index_only_forgery() {
    let mut positives = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            if case.expected.is_some() {
                prove(&observation(&case), &updates(&case)).unwrap();
                positives += 1;
            }
        }
    }
    assert_eq!(positives, 16);
    for recipe in [
        Recipe::Ascending,
        Recipe::Composite,
        Recipe::RestoreOriginal,
    ] {
        let case = model::source(6, recipe);
        let original = observation(&case);
        for relative in [0, 2, 4, 8, 12, 16, 20, 24, 26, 27, 484, 488, 4095] {
            let mut forged = observation(&case);
            forged.after[9 * 4096 + relative] ^= 1;
            assert_eq!(
                prove(&forged, &updates(&case)),
                Err(EditError::Preservation)
            );
        }
        let mut count = observation(&case);
        count.changed_rows = original.changed_rows() + 1;
        assert_eq!(prove(&count, &updates(&case)), Err(EditError::Preservation));
    }
    let case = model::source(6, Recipe::Ascending);
    assert_eq!(
        prove(&observation(&case), &[Int32Update::new(0, 10, 19)]),
        Err(EditError::Preservation)
    );
    assert_eq!(
        prove(&observation(&case), &[Int32Update::new(0, 11, 17)]),
        Err(EditError::Conflict)
    );
}

// Both duplicate keys below are fully self-consistent with the literal final
// rows; a proof that only checked DATA/INDEX agreement would accept them.
fn duplicate_image(before: &[u8]) -> Vec<u8> {
    let mut after = before.to_vec();
    let start = pointer(before, 8, 0);
    assert_eq!(&before[start + 6..start + 10], &10_i32.to_le_bytes());
    after[start + 6..start + 10].copy_from_slice(&20_i32.to_le_bytes());
    let key = 9 * 4096 + 480;
    assert_eq!(&before[key..key + 9], &[0x7f, 0x80, 0, 0, 10, 0, 0, 8, 0]);
    after[key + 4] = 20;
    after
}

fn checked_source(bytes: &[u8]) -> (NativeImage, crate::TableCheck) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap();
    let checked = check_table(
        &mut image.reader(),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    (image, checked)
}

#[test]
fn independent_final_constraints_include_unique_and_primary_logical_aliases() {
    for header in [5, 6] {
        let unique = model::source(header, Recipe::UniqueConflict);
        assert!(unique.expected.is_none());
        checked_source(&unique.before);
        let forged = NativeEditPlan {
            before: unique.before.clone(),
            after: duplicate_image(&unique.before),
            changed_rows: 1,
        };
        assert_eq!(
            prove(&forged, &updates(&unique)),
            Err(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            )))
        );

        let alias = model::source(header, Recipe::Alias);
        let mut before = alias.before.clone();
        // Actual logical descriptor1, type byte+23. Physical UNIQUE remains
        // omitted; the real primary logical alias supplies its stronger rule.
        let kind = 7 * 4096 + 195 + 28 + 23;
        assert_eq!(before[kind], 0);
        assert_eq!(before[7 * 4096 + 143 + 46], 0x80);
        before[kind] = 1;
        let (_, checked) = checked_source(&before);
        assert_eq!(checked.indexes().len(), 2);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.index().constraints().unwrap().unique())
        );
        let forged = NativeEditPlan {
            after: duplicate_image(&before),
            before,
            changed_rows: 1,
        };
        assert_eq!(
            prove(&forged, &[Int32Update::new(0, 10, 9)]),
            Err(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            )))
        );
    }
}

#[test]
fn writable_index_ownership_refuses_actual_map_record_member_and_global_conflicts() {
    let case = model::source(6, Recipe::Ascending);
    let (healthy, checked) = checked_source(&case.before);
    let ownership = Ownership::capture(&healthy, &checked, &mut budget(), &|| false).unwrap();
    ownership
        .require(
            &healthy,
            checked.schema(),
            checked.indexes()[0].index(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    for conflict in 0..4 {
        let mut bytes = case.before.clone();
        if conflict == 0 {
            // Catalog LvProp's actual LVAL owner points at the same map RECORD
            // as selected physical INDEX0, not merely the same DATA container.
            let catalog = 2 * 4096;
            assert_eq!(&bytes[catalog + 8..catalog + 12], &704_u32.to_le_bytes());
            let tail = catalog + 704 - 42;
            assert_eq!(&bytes[tail..tail + 2], &5_u16.to_le_bytes());
            assert_eq!(
                &bytes[tail + 2..tail + 6],
                &((3_u32 << 8) | 8).to_le_bytes()
            );
            bytes[tail + 2..tail + 6].copy_from_slice(&((3_u32 << 8) | 16).to_le_bytes());
        } else if conflict == 1 {
            // Distinct LVAL and INDEX map records now claim the same actual
            // bitmap MEMBER9. Map record exclusivity alone must not accept it.
            let record = pointer(&bytes, 3, 8);
            assert_eq!(&bytes[record..record + 13], &[0; 13]);
            bytes[record + 6] = 2;
        } else if conflict == 2 {
            // The tree/rows remain readable, but GLOBAL declares its leaf free.
            let record = pointer(&bytes, 1, 0);
            assert_eq!(&bytes[record..record + 13], &[0; 13]);
            bytes[record + 6] = 2;
        } else {
            // Insert a real second counter/physical descriptor, shifting the
            // unchanged columns/names/logical descriptor. No logical owner
            // refers to data-number1. This is not a mutated cached schema.
            let td = &bytes[7 * 4096..8 * 4096];
            assert_eq!(&td[8..12], &243_u32.to_le_bytes());
            assert_eq!(&td[47..55], &[1, 0, 0, 0, 1, 0, 0, 0]);
            let mut active = td[..75].to_vec();
            active.extend_from_slice(&[0; 12]);
            active.extend_from_slice(&td[75..195]);
            active.extend_from_slice(&td[143..195]);
            active.extend_from_slice(&td[195..243]);
            assert_eq!(active.len(), 307);
            active[8..12].copy_from_slice(&307_u32.to_le_bytes());
            active[2..4].copy_from_slice(&3781_u16.to_le_bytes());
            active[51..55].copy_from_slice(&2_u32.to_le_bytes());
            bytes[7 * 4096..7 * 4096 + active.len()].copy_from_slice(&active);
        }
        // These are actual source-byte contradictions with a still healthy
        // selected table/index read; no cached TableCheck or schema is mutated.
        let (image, checked) = checked_source(&bytes);
        let error = Ownership::capture(&image, &checked, &mut budget(), &|| false)
            .err()
            .expect("source ownership conflict must refuse");
        if conflict < 2 {
            assert_eq!(error, EditError::Unsupported(WriteBlocker::RowLayout));
        } else if conflict == 2 {
            assert_eq!(error, EditError::Inspect(InspectError::Malformed));
        } else {
            assert_eq!(error, EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        assert_eq!(image.as_bytes(), bytes);
    }
}
