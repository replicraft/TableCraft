//! Explicit increment-policy selection from one immutable captured source.
use super::{IncrementingAutoNumber, counter};
use crate::{
    EditError, NativeImage,
    header::check,
    table::mutation::{gates, preview},
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Select increment-by-one engineering behavior using this image's LAST counter.
    ///
    /// Calling this method explicitly chooses the policy. It neither detects
    /// stored Increment/Random mode nor derives a value from live row IDs.
    /// Existing generated-primary, property and complete before-INSERT gates
    /// apply, including unqualified NewValues refusals and inert default checks.
    ///
    /// The returned policy pins the original counter for
    /// [`Self::preview_ace16_incrementing_insert_rows`], which independently
    /// revalidates it. This helper does not allocate IDs, alter the image or
    /// authorize a normal save, macro execution or reseeding operation.
    pub fn prepare_ace16_incrementing_insert_policy(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<IncrementingAutoNumber, EditError> {
        let _span = tracing::debug_span!("native.insert.increment_policy").entered();
        check(budget, cancelled)?;
        preview::qualify(self)?;
        let checked = gates::before_incrementing_insert(self, table, budget, cancelled)?;
        let source = counter::source(self, &checked, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(IncrementingAutoNumber::new(source.last))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AutoNumberRowInsert, InspectError, WriteBlocker,
        table::mutation::insert::tests::{
            incrementing_autonumber::model::{self, Row, State},
            model as original,
        },
    };
    use std::cell::Cell;
    use tablecraft_core::limits::{Resource, ResourceBudget};
    use tablecraft_testkit::access::insert_seed;

    const LIMITS: [u64; 3] = [256 * 1024 * 1024, 100_000, 100_000_000];

    fn budget() -> ResourceBudget {
        ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2])
    }

    fn states() -> [(State, i32); 8] {
        [
            (State::Fresh, 0),
            (State::HighestDeleted, 3),
            (State::AllDeleted, 3),
            (State::HighWater, 260),
            (State::Negative, -2),
            (State::Minimum, i32::MIN),
            (State::NearMaximum, i32::MAX - 1),
            (State::Maximum, i32::MAX),
        ]
    }

    fn property_source(property: &str, value: &str) -> Vec<u8> {
        let mut case = model::source(6, State::Fresh);
        // Reuse the actual literal catalog/property owner. Its Score group binds
        // the genuine third field in this generated-primary source.
        let catalog = insert_seed::source(6, false, false, Some((property, value)));
        case.before[4 * 4096..5 * 4096].copy_from_slice(&catalog[4 * 4096..5 * 4096]);
        case.before
    }

    #[test]
    fn qualify_increment_policy_sources_and_expected_images_before_helper() {
        for code in [5, 6] {
            for (state, last) in states() {
                let case = model::source(code, state);
                let rows = case
                    .physical_rows
                    .iter()
                    .filter(|(_, _, deleted)| !deleted)
                    .count();
                original::checked(&case.before, rows, 4);
                assert_eq!(
                    &case.before[model::DEFINITION + 20..model::DEFINITION + 24],
                    last.to_le_bytes(),
                );
                // Empty INSERT preserves every byte even at exhausted MAX.
                assert_eq!(model::expected(&case, &[]), case.before);
                if last < i32::MAX {
                    let row = Row {
                        earlier: -7,
                        id: last + 1,
                        score: None,
                    };
                    let expected = model::expected(&case, &[row]);
                    original::checked(&expected, rows + 1, 4);
                }
            }
        }
    }

    #[test]
    fn qualify_increment_policy_property_sources_before_helper() {
        for (property, value) in [("NewValues", "Random"), ("DefaultValue", "0")] {
            let bytes = property_source(property, value);
            original::checked(&bytes, 0, 4);
            let properties = crate::inspect_table_properties(
                &mut std::io::Cursor::new(&bytes),
                tablecraft_model::CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let group = properties
                .groups()
                .iter()
                .find(|group| group.name() == "Score")
                .unwrap();
            assert_eq!(group.get(property).unwrap().text().unwrap(), value);
        }
    }

    #[test]
    fn unqualified_mode_remains_refused_while_companion_defaults_stay_inert() {
        let bytes = property_source("NewValues", "Random");
        let image = original::capture(&bytes);
        assert!(matches!(
            image.prepare_ace16_incrementing_insert_policy("Items", &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Properties))
        ));
        assert_eq!(image.as_bytes(), bytes);
        let bytes = property_source("DefaultValue", "0");
        let image = original::capture(&bytes);
        let policy = image
            .prepare_ace16_incrementing_insert_policy("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(policy.expected_last(), 0);
        assert_eq!(image.as_bytes(), bytes);
    }

    #[test]
    fn policy_uses_signed_last_not_live_max_and_feeds_existing_exact_preview() {
        for code in [5, 6] {
            for (state, last) in states() {
                let case = model::source(code, state);
                let image = original::capture(&case.before);
                let policy = image
                    .prepare_ace16_incrementing_insert_policy("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(policy.expected_last(), last);
                assert_eq!(image.as_bytes(), case.before);
                let additions = if last == i32::MAX {
                    Vec::new()
                } else {
                    vec![Row {
                        earlier: -7,
                        id: last + 1,
                        score: None,
                    }]
                };
                let expected = model::expected(&case, &additions);
                let requests = additions
                    .iter()
                    .map(|row| AutoNumberRowInsert::new(row.companions()))
                    .collect::<Vec<_>>();
                let candidate = image
                    .preview_ace16_incrementing_insert_rows(
                        "Items",
                        &requests,
                        policy,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(candidate.candidate_bytes(), expected);
            }
        }
    }

    #[test]
    fn helper_keeps_generated_profile_and_explicit_counter_preconditions() {
        let case = model::source(6, State::Fresh);
        let mut ordinary = case.before.clone();
        ordinary[model::GENERATED_FLAGS] = 3;
        assert!(matches!(
            original::capture(&ordinary).prepare_ace16_incrementing_insert_policy(
                "Items",
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        let image = original::capture(&case.before);
        let policy = image
            .prepare_ace16_incrementing_insert_policy("Items", &mut budget(), &|| false)
            .unwrap();
        let next = model::source(6, State::HighWater);
        assert!(matches!(
            original::capture(&next.before).preview_ace16_incrementing_insert_rows(
                "Items",
                &[],
                policy,
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Conflict)
        ));
        assert!(matches!(
            image.preview_ace16_incrementing_insert_rows(
                "Items",
                &[],
                IncrementingAutoNumber::new(1),
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Conflict)
        ));
    }

    #[test]
    fn helper_charges_all_pools_and_observes_the_actual_final_poll() {
        let case = model::source(6, State::HighWater);
        let image = original::capture(&case.before);
        let polls = Cell::new(0);
        let mut healthy = budget();
        image
            .prepare_ace16_incrementing_insert_policy("Items", &mut healthy, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
        for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
            .into_iter()
            .enumerate()
        {
            let used = LIMITS[position] - healthy.remaining(resource);
            assert!(used > 0);
            let mut limits = LIMITS;
            limits[position] = used - 1;
            let mut limited = ResourceBudget::new(limits[0], limits[1], limits[2]);
            assert!(matches!(
                image.prepare_ace16_incrementing_insert_policy("Items", &mut limited, &|| false),
                Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource
            ));
        }
        let terminal = polls.get();
        polls.set(0);
        assert!(matches!(
            image.prepare_ace16_incrementing_insert_policy("Items", &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            }),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
        assert_eq!(polls.get(), terminal);
        assert_eq!(image.as_bytes(), case.before);
    }
}
