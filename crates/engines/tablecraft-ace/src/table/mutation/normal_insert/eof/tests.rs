//! Literal complete images and independent direct proof; no intent/host authority.
use super::{OrdinaryInsertPlanKind, proof};
use crate::{EditError, InspectError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker};
use sha2::{Digest, Sha256};
use std::{array, cell::Cell, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(194);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

struct Case {
    before: Vec<u8>,
    after: Vec<u8>,
    batch: bool,
    definition: usize,
    data: usize,
    maps: usize,
    bitmap: usize,
    eof: usize,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap()
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (variable, batch, entries) in [
        (
            "TABLECRAFT_NORMAL_INSERT_EOF_SOURCE_DIR",
            true,
            [
                (
                    5,
                    458752,
                    107,
                    110,
                    108,
                    111,
                    112,
                    "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
                    "6d01ee69441a765e2dfe28c4affcf9831b90f21b6839ecb582e7232ac6d80cc1",
                ),
                (
                    6,
                    528384,
                    125,
                    127,
                    126,
                    128,
                    129,
                    "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
                    "316adbe11b900e8abd26cbf385a738770622ea39b41b4181f31de43c243fc3e7",
                ),
            ],
        ),
        (
            "TABLECRAFT_NORMAL_SINGLE_INSERT_EOF_SOURCE_DIR",
            false,
            [
                (
                    5,
                    458752,
                    107,
                    110,
                    108,
                    111,
                    112,
                    "9934f7d78854336521bfed4375712fd8ffad283855ae14ee8b3d833c681ff3fa",
                    "91bfe1d2332e2173f88810fea2053ee1ce531b3f7019692eedc41339b2b12b2e",
                ),
                (
                    6,
                    528384,
                    125,
                    127,
                    126,
                    128,
                    129,
                    "3737bed0bc09ac9aa2f4fb780d5bbf771dbd5134c8f4170e12b82ce84b563272",
                    "3c2e562e8cfc91bf299e9fce6394bddfd9762e5ce25aa05a9d97171072611a4d",
                ),
            ],
        ),
    ] {
        let directory = PathBuf::from(std::env::var_os(variable).unwrap());
        assert!(directory.is_absolute());
        for (header, bytes, definition, data, maps, bitmap, eof, before_hash, after_hash) in entries
        {
            let prefix = if batch {
                format!("h{header}-insert-eof")
            } else {
                format!("h{header}-single-insert-eof")
            };
            let before = fs::read(directory.join(format!("{prefix}-before.accdb"))).unwrap();
            let after = fs::read(directory.join(format!("{prefix}-after.accdb"))).unwrap();
            for (raw, size, expected) in [
                (&before, bytes, before_hash),
                (&after, bytes + 4096, after_hash),
            ] {
                assert_eq!(raw.len(), size);
                let actual = Sha256::digest(raw)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                assert_eq!(actual, expected);
            }
            cases.push(Case {
                before,
                after,
                batch,
                definition,
                data,
                maps,
                bitmap,
                eof,
            });
        }
    }
    cases
}

fn requests(batch: bool) -> Vec<RowInsert> {
    let ids = if batch { 24..=278 } else { 260..=260 };
    ids.map(|id| RowInsert::new(vec![Value::Int32(id), Value::Null, Value::Null]))
        .collect()
}

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    image(&case.before).plan_ordinary_eof_row_inserts(
        "Sentinel",
        &requests(case.batch),
        work,
        cancelled,
    )
}

fn inspection(error: EditError) -> InspectError {
    match error {
        EditError::Inspect(error) => error,
        other => panic!("expected resource/cancel error, got {other:?}"),
    }
}

#[test]
#[ignore = "requires both eight-view source native/SDK qualifications"]
fn literal_batch_and_single_eof_insert_and_metadata_selection_match_independent_images() {
    for case in cases() {
        let original = case.before.clone();
        assert_eq!(
            image(&case.before)
                .ordinary_insert_plan_kind(
                    "Sentinel",
                    &requests(case.batch),
                    &mut budget(),
                    &|| false
                )
                .unwrap(),
            OrdinaryInsertPlanKind::AllocateEof
        );
        let actual = plan(&case, &mut budget(), &|| false).unwrap();
        assert_eq!(actual.before_bytes(), case.before);
        assert_eq!(actual.after_bytes(), case.after);
        assert_eq!(actual.changed_rows(), if case.batch { 255 } else { 1 });
        assert_eq!(case.before, original);
        assert_eq!(
            image(&case.before)
                .ordinary_insert_plan_kind("Sentinel", &[], &mut budget(), &|| false)
                .unwrap(),
            OrdinaryInsertPlanKind::Existing
        );
        assert_eq!(
            image(&case.before)
                .plan_ordinary_eof_row_inserts("Sentinel", &[], &mut budget(), &|| false)
                .err()
                .unwrap(),
            EditError::Unsupported(WriteBlocker::RowLayout)
        );
        if case.batch {
            let fitting = vec![RowInsert::new(vec![
                Value::Int32(24),
                Value::Null,
                Value::Null,
            ])];
            assert_eq!(
                image(&case.before)
                    .ordinary_insert_plan_kind("Sentinel", &fitting, &mut budget(), &|| false)
                    .unwrap(),
                OrdinaryInsertPlanKind::Existing
            );
            assert_eq!(
                image(&case.before)
                    .plan_ordinary_eof_row_inserts("Sentinel", &fitting, &mut budget(), &|| false)
                    .err()
                    .unwrap(),
                EditError::Unsupported(WriteBlocker::RowLayout)
            );
        }
    }
}

#[test]
#[ignore = "requires both eight-view source native/SDK qualifications"]
fn direct_literal_eof_proof_rejects_map_visibility_count_null_and_protected_complement_forgeries() {
    let mut proved = 0;
    for case in cases() {
        let count = if case.batch { 255 } else { 1 };
        let literal = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: count,
        };
        proof::prove(
            &literal,
            SCOPE,
            "Sentinel",
            &requests(case.batch),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let cells = [
            4096 + 4032 + case.eof / 8,
            case.bitmap * 4096 + 4 + case.eof / 8,
            case.maps * 4096 + 3963 + case.eof / 8,
        ];
        let first = case.eof * 4096 + 4081;
        for offset in [
            cells[0],
            cells[1],
            cells[2],
            case.eof * 4096 + 14,
            first + 14,
            case.eof * 4096 + 8,
            case.definition * 4096 + 16,
            case.definition * 4096 + 20,
            case.data * 4096 + 4095,
        ] {
            let mut changed = case.after.clone();
            changed[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone().into(),
                after: changed,
                changed_rows: count,
            };
            assert!(matches!(
                proof::prove(
                    &forged,
                    SCOPE,
                    "Sentinel",
                    &requests(case.batch),
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
        }
        let wrong = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        assert!(matches!(
            proof::prove(
                &wrong,
                SCOPE,
                "Sentinel",
                &requests(case.batch),
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        ));
        proved += 1;
    }
    assert_eq!(proved, 4);
}

#[test]
#[ignore = "requires both eight-view source native/SDK qualifications"]
fn full_eof_insert_pays_exact_resource_pools_and_every_real_trace_cancellation_boundary() {
    for case in cases() {
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        let actual = plan(&case, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(actual.after_bytes(), case.after);
        let costs: [u64; 3] = array::from_fn(|i| initial[i] - measured.remaining(POOLS[i]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        assert_eq!(
            plan(&case, &mut exact, &|| false).unwrap().after_bytes(),
            case.after
        );
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
        for i in 0..3 {
            let mut limit = costs;
            limit[i] -= 1;
            let error = plan(
                &case,
                &mut ResourceBudget::new(limit[0], limit[1], limit[2]),
                &|| false,
            )
            .err()
            .unwrap();
            assert!(
                matches!(inspection(error), InspectError::Limit(error) if error.resource == POOLS[i])
            );
        }
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            let error = plan(&case, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .unwrap();
            assert_eq!(inspection(error), InspectError::Cancelled);
            assert_eq!(seen.get(), stop);
        }
    }
}
