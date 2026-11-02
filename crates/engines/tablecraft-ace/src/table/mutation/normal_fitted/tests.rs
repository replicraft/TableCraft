//! Qualified independent SOURCE images authorize tests, never producer output.
use super::proof;
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeEditPlan, NativeFittedIntentGroup,
    NativeFittedUpdates, NativeImage, NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, SlotScalarUpdate, WriteBlocker, read_table,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_fitted::{
    self as model, Case, Recipe, Refusal,
};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];
const SCOPE: CatalogScope = CatalogScope::new(129);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFittedIntentGroup> {
    case.groups
        .iter()
        .map(|group| {
            let updates = if matches!(group.number, 1 | 2) {
                NativeFittedUpdates::Variable(
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            SlotScalarUpdate::new(
                                update.row,
                                update.expected.clone(),
                                update.replacement.clone(),
                            )
                        })
                        .collect(),
                )
            } else {
                NativeFittedUpdates::Fixed(
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            FixedScalarUpdate::new(
                                update.row,
                                update.expected.clone(),
                                update.replacement.clone(),
                            )
                        })
                        .collect(),
                )
            };
            NativeFittedIntentGroup::new(group.column, updates, &mut budget(), &|| false).unwrap()
        })
        .collect()
}

fn literal_plan(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone(),
        after: case
            .expected
            .as_ref()
            .expect("positive SOURCE outcome")
            .clone(),
        changed_rows: case.changed,
    }
}

fn prove(case: &Case, plan: &NativeEditPlan) -> Result<(), EditError> {
    let original = crate::NativeFittedEditIntent::new(
        "Items",
        groups(case),
        &case.before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let borrowed = original.borrowed_groups(&mut budget(), &|| false).unwrap();
    proof::prove(plan, SCOPE, "Items", &borrowed, &mut budget(), &|| false)
}

#[test]
fn ordinary_fitted_public_plans_match_thirty_literal_images_and_retain_tcni5_requests() {
    let mut outcomes = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("{}: {error:?}", recipe.id()));
            assert_eq!(plan.before_bytes(), case.before, "{}", recipe.id());
            assert_eq!(plan.after_bytes(), expected, "{}", recipe.id());
            assert_eq!(plan.changed_rows(), case.changed, "{}", recipe.id());
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFitted
            );
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(&wire[..8], b"TCNI0005");
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
            let fitted = decoded.fitted().unwrap();
            fitted
                .verify_before(&case.before, &mut budget(), &|| false)
                .unwrap();
            let borrowed = fitted.borrowed_groups(&mut budget(), &|| false).unwrap();
            assert_eq!(borrowed.len(), case.groups.len());
            for (actual, supplied) in borrowed.iter().zip(&case.groups) {
                assert_eq!(actual.name(), supplied.column);
                assert_eq!(actual.len(), supplied.updates.len());
                assert_eq!(
                    matches!(actual, crate::FittedColumnUpdates::Variable { .. }),
                    matches!(supplied.number, 1 | 2)
                );
                for (index, supplied) in supplied.updates.iter().enumerate() {
                    let actual = actual.cell(index).unwrap();
                    assert_eq!(actual.row, supplied.row);
                    assert_eq!(actual.expected, &supplied.expected);
                    assert_eq!(actual.value, &supplied.replacement);
                }
            }
            outcomes += 1;
        }
    }
    assert_eq!(outcomes, 30);
}

#[test]
fn ordinary_fitted_capacity_and_conflict_sources_refuse_with_the_original_typed_errors() {
    let mut refusals = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(refusal) = case.refusal else {
                continue;
            };
            let error = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("negative SOURCE outcome");
            assert_eq!(
                error,
                RecoverableEditError::Edit(match refusal {
                    Refusal::Capacity => EditError::Unsupported(WriteBlocker::RowLayout),
                    Refusal::Conflict => EditError::Conflict,
                }),
                "{}",
                recipe.id()
            );
            refusals += 1;
        }
        let case = model::source(header, Recipe::EmptyGroups);
        for column in ["Missing", "DecimalValue", "Id"] {
            let original = vec![
                NativeFittedIntentGroup::new(
                    column,
                    NativeFittedUpdates::Variable(Vec::new()),
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            ];
            let error = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates("Items", original, &mut budget(), &|| {
                    false
                })
                .err()
                .expect("empty groups still qualify actual target metadata");
            assert_eq!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column))
            );
        }
        let mut counter = tablecraft_testkit::fixtures::access::canonical_ordinary_indexed::source(
            tablecraft_testkit::fixtures::access::canonical_ordinary_indexed::Case::VariablePrimary,
        );
        counter.bytes[20] = header;
        let original = vec![
            NativeFittedIntentGroup::new(
                "TextValue",
                NativeFittedUpdates::Variable(Vec::new()),
                &mut budget(),
                &|| false,
            )
            .unwrap(),
            NativeFittedIntentGroup::new(
                "Id",
                NativeFittedUpdates::Fixed(Vec::new()),
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        ];
        let error = image(&counter.bytes)
            .plan_recoverable_ordinary_fitted_updates("Items", original, &mut budget(), &|| false)
            .err()
            .expect("empty generated target remains read-only");
        assert_eq!(
            error,
            RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column))
        );
    }
    assert_eq!(refusals, 8);
}

#[test]
fn ordinary_fitted_fresh_proof_accepts_thirty_source_images_without_a_producer() {
    let mut outcomes = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            if case.expected.is_none() {
                continue;
            }
            prove(&case, &literal_plan(&case))
                .unwrap_or_else(|error| panic!("{}: {error:?}", recipe.id()));
            outcomes += 1;
        }
    }
    assert_eq!(outcomes, 30);
}

#[test]
fn ordinary_fitted_proof_refuses_siblings_null_payload_padding_footer_counts_and_encoding_aliases()
{
    for header in [5, 6] {
        let joint = model::source(header, Recipe::JointFiveFixed);
        let frame = &joint.frames[0];
        for at in [
            frame.absolute + 52, // Unselected Decimal follows beta and sixteen Binary bytes.
            frame.absolute + 2,  // Fixed Id sibling of other requested cells.
            frame.absolute + 14, // Money is NULL; retain its inactive payload.
            frame.absolute + frame.length - 1, // Shared presence/Boolean mask.
            frame.absolute + frame.length - 5, // Variable EOD boundary.
            joint.frames[2].absolute + 22, // Unselected row's DATE payload.
            7 * 4096 + 20,       // Untouched historical LAST storage.
        ] {
            let mut forged = literal_plan(&joint);
            forged.after[at] ^= 1;
            assert!(prove(&joint, &forged).is_err(), "forged byte {at}");
        }
        let mut wrong_count = literal_plan(&joint);
        wrong_count.changed_rows += 1;
        assert!(prove(&joint, &wrong_count).is_err());
        wrong_count.changed_rows = 0;
        assert!(prove(&joint, &wrong_count).is_err());

        let beta = model::source(header, Recipe::CompressedText);
        let frame = &beta.frames[0];
        let mut padding = literal_plan(&beta);
        padding.after[frame.absolute + frame.data_end - 1] ^= 1;
        assert!(prove(&beta, &padding).is_err());
        let mut alternate = literal_plan(&beta);
        let row = &mut alternate.after[frame.absolute..frame.absolute + frame.length];
        // Literal raw UTF16 beta has the same value as the qualified compressed
        // form, but is not the declared fitting wire selected by this request.
        row[30..38].copy_from_slice(&[98, 0, 101, 0, 116, 0, 97, 0]);
        row[38..42].copy_from_slice(&[0, 255, 1, 2]);
        row[42..59].copy_from_slice(&beta.before[frame.absolute + 44..frame.absolute + 61]);
        row[59..69].fill(0xa5);
        for (index, offset) in [30_u16, 38, 42, 59].into_iter().enumerate() {
            let at = frame.length - 3 - 2 * (index + 1);
            row[at..at + 2].copy_from_slice(&offset.to_le_bytes());
        }
        let observed = read_table(
            &mut Cursor::new(&alternate.after),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(observed.rows(), beta.final_rows);
        assert!(prove(&beta, &alternate).is_err());
    }
}

fn inspect_error(error: RecoverableEditError) -> InspectError {
    match error {
        RecoverableEditError::Edit(EditError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Request(EditError::Inspect(error))) => {
            error
        }
        other => panic!("unexpected refusal: {other:?}"),
    }
}

#[test]
fn ordinary_fitted_planning_pays_exact_pools_and_cancels_at_entry_middle_and_terminal() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::JointFiveFixed);
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "Items",
                groups(&case),
                &mut measured,
                &|| false,
            )
            .unwrap();
        let costs: [u64; 3] =
            std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        image(&case.before)
            .plan_recoverable_ordinary_fitted_updates("Items", groups(&case), &mut exact, &|| false)
            .unwrap();
        for pool in POOLS {
            assert_eq!(exact.remaining(pool), 0);
        }
        for (index, pool) in POOLS.into_iter().enumerate() {
            let mut limits = costs;
            limits[index] -= 1;
            let error = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(&case),
                    &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                    &|| false,
                )
                .err()
                .expect("incomplete work cannot return a recoverable plan");
            assert!(
                matches!(inspect_error(error), InspectError::Limit(error) if error.resource == pool)
            );
        }
        let polls = Cell::new(0_usize);
        image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "Items",
                groups(&case),
                &mut budget(),
                &|| {
                    polls.set(polls.get() + 1);
                    false
                },
            )
            .unwrap();
        let terminal = polls.get();
        assert!(terminal > 2);
        for stop in [1, terminal / 2, terminal] {
            polls.set(0);
            let error = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| {
                        polls.set(polls.get() + 1);
                        polls.get() == stop
                    },
                )
                .err()
                .expect("cancelled work cannot return a recoverable plan");
            assert_eq!(inspect_error(error), InspectError::Cancelled);
            assert_eq!(polls.get(), stop);
        }
    }
}

#[test]
fn readonly_counter_with_only_variable_targets_uses_fitted_capability_and_mask_only_empty_text() {
    use tablecraft_core::values::Value;
    use tablecraft_testkit::fixtures::access::canonical_variable_creation::{
        self as variable, Case,
    };

    for header in [5, 6] {
        let source = variable::source(Case::NullAndEmpty);
        let initial = read_table(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(initial.rows(), source.rows);

        // New authored witness: the existing five-field fixture contains a
        // writable Flag. Remove only its zero-width declaration/name and mask
        // contribution, leaving Id plus the three variable declarations. This
        // is not a claim about stripping or qualifying an imported real file.
        let mut before = source.bytes.clone();
        before[20] = header;
        let td = 7 * 4096;
        assert_eq!(
            u32::from_le_bytes(before[td + 8..td + 12].try_into().unwrap()),
            276
        );
        assert_eq!(&before[td + 41..td + 47], &[5, 0, 3, 0, 5, 0]);
        assert_eq!(before[td + 63], 4);
        assert_eq!(before[td + 78], 3);
        assert_eq!(&before[td + 86..td + 88], &[4, 0]);
        assert_eq!(before[td + 163], 1);
        assert_eq!(before[td + 178], 3);
        assert_eq!(&before[td + 186..td + 188], &[0, 0]);
        assert_eq!(
            &before[td + 264..td + 274],
            &[8, 0, 70, 0, 108, 0, 97, 0, 103, 0]
        );
        assert_eq!(&before[td + 274..td + 276], &[255, 255]);
        assert_eq!(before[td + 103], 2);
        assert_eq!(
            before[td + 104],
            0,
            "Text has no Unicode-compression preference"
        );
        assert_eq!(&before[td + 20..td + 24], &[0, 0, 0, 0]);
        before.copy_within(td + 188..td + 264, td + 163);
        before[td + 239..td + 4096].fill(0);
        before[td + 239..td + 241].copy_from_slice(&[255, 255]);
        before[td + 8..td + 12].copy_from_slice(&241_u32.to_le_bytes());
        before[td + 2..td + 4].copy_from_slice(&3847_u16.to_le_bytes());
        before[td + 41..td + 43].copy_from_slice(&4_u16.to_le_bytes());
        before[td + 45..td + 47].copy_from_slice(&4_u16.to_le_bytes());
        before[td + 78] = 7;
        before[td + 20..td + 24].copy_from_slice(&4_i32.to_le_bytes());

        let data = 8 * 4096;
        let bounds = [
            (4079_usize, 4096_usize),
            (4045, 4079),
            (4028, 4045),
            (4011, 4028),
        ];
        let masks = [0x01_u8, 0x1f, 0x15, 0x03];
        for (row, &(start, end)) in bounds.iter().enumerate() {
            let directory = data + 14 + 2 * row;
            assert_eq!(
                u16::from_le_bytes(before[directory..directory + 2].try_into().unwrap()),
                start as u16
            );
            if row != 0 {
                assert_eq!(
                    u16::from_le_bytes(before[directory - 2..directory].try_into().unwrap()),
                    end as u16
                );
            }
            assert_eq!(&before[data + start..data + start + 2], &[5, 0]);
            assert_eq!(before[data + end - 1], masks[row]);
            before[data + start..data + start + 2].copy_from_slice(&4_u16.to_le_bytes());
            before[data + end - 1] &= !0x10;
        }
        let mut original_rows = source.rows.clone();
        for row in &mut original_rows {
            row.truncate(4);
        }
        assert_eq!(original_rows[0][1], Value::Null);

        // The NULL row has no variable payload: four literal reverse offset
        // words are 6 and the variable count is 3. Empty raw Text needs no bytes.
        assert_eq!(
            &before[data + 4079 + 6..data + 4095],
            &[6, 0, 6, 0, 6, 0, 6, 0, 3, 0]
        );
        assert_eq!(before[data + 4095], 1);
        let mut expected = before.clone();
        expected[data + 4095] = 3;
        let mut expected_rows = original_rows.clone();
        expected_rows[0][1] = Value::Text(String::new());

        // Read and constraint qualification of both independently authored
        // complete images precedes the first producer call on this witness.
        for (bytes, rows) in [(&before, &original_rows), (&expected, &expected_rows)] {
            let observed = read_table(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(observed.rows(), *rows);
            let checked = crate::check_table(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 4);
            assert!(checked.indexes().is_empty());
            assert_eq!(&bytes[td + 20..td + 24], &4_i32.to_le_bytes());
            assert_eq!(bytes[td + 78], 7);
        }
        let original = image(&before);
        assert_eq!(
            original.check_native_ordinary_fixed_table("Items", &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        );
        assert!(
            original
                .select_native_ordinary_fitted_table("Items", &mut budget(), &|| false)
                .unwrap()
        );
        original
            .check_native_ordinary_fitted_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(original.as_bytes(), before);

        let original_groups = vec![
            NativeFittedIntentGroup::new(
                "TextValue",
                NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                    0,
                    Value::Null,
                    Value::Text(String::new()),
                )]),
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        ];
        let plan = original
            .plan_recoverable_ordinary_fitted_updates(
                "Items",
                original_groups,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryFitted
        );
    }
}
