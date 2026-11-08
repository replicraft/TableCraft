//! Source-backed two-page placement, direct proof and original TCNA4 controls.
use super::proof;
use crate::{
    EditError, InspectError, NativeCompleteInsertIntent, NativeEditPlan, NativeImage, RowInsert,
};
use sha2::{Digest, Sha256};
use std::{array, cell::Cell, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(195);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

struct Case {
    header: u8,
    before: Vec<u8>,
    after: Vec<u8>,
    definition: usize,
    root: usize,
    eof: usize,
    maps: usize,
    bitmap: usize,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 2_000_000, 2_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cases() -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_MULTIPAGE_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    [
        (
            5,
            458752,
            107,
            110,
            112,
            108,
            111,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "1f89ca37898bb5374331c673d633b70d06c9f2c75f3b4458a7647f743fb552db",
        ),
        (
            6,
            528384,
            125,
            127,
            129,
            126,
            128,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "d9ea6bfbea2b0d6dd0c95a4cfd543cda2ad4a2dd57523f5ff7c7c34847e544d0",
        ),
    ]
    .into_iter()
    .map(
        |(header, length, definition, root, eof, maps, bitmap, before_hash, after_hash)| {
            let before =
                fs::read(directory.join(format!("h{header}-insert-multipage-before.accdb")))
                    .unwrap();
            let after = fs::read(directory.join(format!("h{header}-insert-multipage-after.accdb")))
                .unwrap();
            assert_eq!(before.len(), length);
            assert_eq!(after.len(), length + 8192);
            assert_eq!(digest(&before), before_hash);
            assert_eq!(digest(&after), after_hash);
            Case {
                header,
                before,
                after,
                definition,
                root,
                eof,
                maps,
                bitmap,
            }
        },
    )
    .collect()
}

fn requests() -> Vec<RowInsert> {
    (24..624)
        .map(|id| RowInsert::new(vec![Value::Int32(id), Value::Null, Value::Null]))
        .collect()
}

fn physical(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    image(&case.before).plan_ordinary_complete_row_inserts("Sentinel", &requests(), work, cancelled)
}

#[test]
#[ignore = "requires four literal source views with native and independent SDK qualifications"]
fn two_page_complete_rows_match_literal_images_and_original_tcna4_wires() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_MULTIPAGE_INSERT_INTENTS_DIR").unwrap());
    assert!(directory.is_absolute());
    for case in cases() {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                "Sentinel",
                requests(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 600);
        assert_eq!(plan.intent().rows().len(), 600);
        for (position, row) in plan.intent().rows().iter().enumerate() {
            assert_eq!(
                row.values(),
                &[Value::Int32(24 + position as i32), Value::Null, Value::Null]
            );
        }
        let wire = fs::read(directory.join(format!("h{}-batch600.intent", case.header))).unwrap();
        assert_eq!(wire.len(), 21676);
        assert_eq!(&wire[..8], b"TCNA0004");
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire
        );
        let decoded = NativeCompleteInsertIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        decoded
            .verify_before(&case.before, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    }
}

#[test]
#[ignore = "requires four literal source views with native and independent SDK qualifications"]
fn fresh_direct_proof_rejects_extra_page_each_map_claim_visibility_and_protected_bytes() {
    for case in cases() {
        let literal = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: 600,
        };
        proof::prove(
            &literal,
            SCOPE,
            "Sentinel",
            &requests(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let cells = [
            4096 + 4032 + case.eof / 8,
            case.bitmap * 4096 + 4 + case.eof / 8,
            case.maps * 4096 + 3963 + case.eof / 8,
        ];
        for bit in [case.eof, case.eof + 1] {
            for at in cells {
                let mut after = case.after.clone();
                after[at] ^= 1 << (bit % 8);
                let forged = NativeEditPlan {
                    before: case.before.clone().into(),
                    after,
                    changed_rows: 600,
                };
                assert_eq!(
                    proof::prove(
                        &forged,
                        SCOPE,
                        "Sentinel",
                        &requests(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(EditError::Preservation)
                );
            }
        }
        for at in [
            case.definition * 4096 + 16,
            case.definition * 4096 + 20,
            case.root * 4096 + 4095,
            case.eof * 4096 + 14,
            (case.eof + 1) * 4096 + 4095,
            (case.eof + 1) * 4096 + 2236 + 14,
        ] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone().into(),
                after,
                changed_rows: 600,
            };
            assert_eq!(
                proof::prove(
                    &forged,
                    SCOPE,
                    "Sentinel",
                    &requests(),
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
        }
        let mut after = case.after.clone();
        after.resize(after.len() + 4096, 0);
        let extra = NativeEditPlan {
            before: case.before.clone().into(),
            after,
            changed_rows: 600,
        };
        assert_eq!(
            proof::prove(
                &extra,
                SCOPE,
                "Sentinel",
                &requests(),
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
}

#[test]
#[ignore = "requires four literal source views with native and independent SDK qualifications"]
fn complete_batch_exact_pools_and_recorded_cancellation_boundaries_precede_effects() {
    for case in cases() {
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        assert_eq!(
            physical(&case, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap()
            .after_bytes(),
            case.after
        );
        let costs: [u64; 3] = array::from_fn(|i| initial[i] - measured.remaining(POOLS[i]));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        assert_eq!(
            physical(&case, &mut exact, &|| false)
                .unwrap()
                .after_bytes(),
            case.after
        );
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
        for i in 0..3 {
            let mut limits = costs;
            limits[i] -= 1;
            let error = physical(
                &case,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false,
            )
            .err()
            .unwrap();
            assert!(
                matches!(error, EditError::Inspect(InspectError::Limit(error)) if error.resource == POOLS[i])
            );
        }
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            let error = physical(&case, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .unwrap();
            assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
            assert_eq!(seen.get(), stop);
        }
        assert_eq!(
            digest(&case.before),
            if case.header == 5 {
                "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4"
            } else {
                "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d"
            }
        );
    }
}
