//! Complete literal AFTERs enter the independent proof without producer staging.
use super::prove;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_indexed_guid_update::requests as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "28 original ordered requests; independent full DATA/index/history/map preservation"]
fn indexed_guid_literal_populations_and_protected_bytes_have_fresh_proof() {
    let cases = source::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut counts = [0; 4];
    for case in cases {
        let updates: Vec<_> = case
            .updates()
            .into_iter()
            .map(|(row, before, after)| FixedScalarUpdate::new(row, before, after))
            .collect();
        let groups = [FixedColumnUpdates::new("Items", "Group", &updates)];
        let plan = NativeEditPlan {
            before: case.before.to_vec().into(),
            after: case.after.to_vec(),
            changed_rows: case.changed_rows(),
        };
        prove(
            &plan,
            CatalogScope::new(298),
            "Items",
            &groups,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        counts[0] += 1;

        let wrong_count = NativeEditPlan {
            before: case.before.to_vec().into(),
            after: case.after.to_vec(),
            changed_rows: case.changed_rows() + 1,
        };
        assert_eq!(
            prove(
                &wrong_count,
                CatalogScope::new(298),
                "Items",
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        counts[1] += 1;

        // Reverse swap has the same observed candidate and protected-byte roles.
        if !case.reverse {
            for (offset, mask) in case.forgeries() {
                let mut after = case.after.to_vec();
                after[offset] ^= mask;
                let forged = NativeEditPlan {
                    before: case.before.to_vec().into(),
                    after,
                    changed_rows: case.changed_rows(),
                };
                assert_eq!(
                    prove(
                        &forged,
                        CatalogScope::new(298),
                        "Items",
                        &groups,
                        &mut budget(),
                        &|| false
                    ),
                    Err(EditError::Preservation),
                    "h{} descending={} kind={} offset={offset}",
                    case.header,
                    case.descending,
                    case.kind
                );
                counts[2] += 1;
            }
        }
        if case.kind == 1 {
            let score = [FixedScalarUpdate::new(
                0,
                Value::Int32(-64),
                Value::Int32(-63),
            )];
            for reverse in [false, true] {
                let guid = FixedColumnUpdates::new("Items", "Group", &updates);
                let score = FixedColumnUpdates::new("Items", "Score", &score);
                let groups = if reverse {
                    [score, guid]
                } else {
                    [guid, score]
                };
                let composed = NativeEditPlan {
                    before: case.before.to_vec().into(),
                    after: case.composite_after(),
                    changed_rows: 1,
                };
                prove(
                    &composed,
                    CatalogScope::new(298),
                    "Items",
                    &groups,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                counts[3] += 1;
            }
        }
    }
    assert_eq!(counts, [28, 28, 468, 8]);
}
