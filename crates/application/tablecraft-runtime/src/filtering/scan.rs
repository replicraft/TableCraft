//! Complete eager predicate traversal, shared by initial and explicitly rebound filters.
use super::{Criterion, FilterError, Predicate, criteria, patterns};
use crate::{Error, controls::check};
use tablecraft_access::{NativeBrowser, NativeCollationResolver};
use tablecraft_ace::{NativeReadSource, TableSchema};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{
    logic::Truth,
    predicates::{LikeMode, PreparedLikeSearch},
};

const SCAN_BATCH_ROWS: u32 = 100;
const INITIAL_MAP_CAPACITY: usize = 64;

pub(super) fn membership<R: NativeReadSource>(
    browser: &mut NativeBrowser<'_, R>,
    schema: &TableSchema,
    predicates: &[Criterion],
    resolver: &NativeCollationResolver<'_>,
    mode: Option<LikeMode>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u32>, u32), Error> {
    let mut positions = Vec::new();
    work.charge_allocation(predicates.len() as u64, size_of::<usize>() as u64)?;
    positions.try_reserve_exact(predicates.len())?;
    for criterion in predicates {
        let index = criteria::field_position(schema, &criterion.column, work, cancelled)?;
        criteria::require_field(&schema.columns()[index])?;
        positions.push(index);
    }

    // Compile patterns once for this complete scan. Retained criteria remain
    // source-free preferences, so a reapply resolves the fresh database mode.
    let mut pattern_work = ResourceBudget::new(patterns::PATTERN_BYTES, 0, patterns::PATTERN_STEPS);
    pattern_work.charge_allocation(
        predicates.len() as u64,
        size_of::<Option<PreparedLikeSearch>>() as u64,
    )?;
    let mut prepared = Vec::new();
    prepared.try_reserve_exact(predicates.len())?;
    for (criterion, &position) in predicates.iter().zip(&positions) {
        if matches!(criterion.predicate, Predicate::Pattern { .. }) {
            let policy = match schema.columns()[position].text_sort_order() {
                Some(order) => resolver.resolve_native(order)?,
                None => resolver.default_collation(),
            };
            patterns::qualify(policy)?;
        }
        prepared.push(patterns::compile(
            criterion,
            mode,
            &mut pattern_work,
            cancelled,
        )?);
    }

    work.charge_allocation(positions.len() as u64, size_of::<usize>() as u64)?;
    let mut projection = Vec::new();
    projection.try_reserve_exact(positions.len())?;
    projection.extend_from_slice(&positions);
    projection.sort_unstable();
    projection.dedup();
    work.charge_allocation(projection.len() as u64, size_of::<&str>() as u64)?;
    let mut names = Vec::new();
    names.try_reserve_exact(projection.len())?;
    for &position in &projection {
        names.push(schema.columns()[position].name());
    }

    let mut cursor = browser.projected_cursor(schema.name(), &names)?;
    for (criterion, &position) in predicates.iter().zip(&positions) {
        let projected = projection
            .binary_search(&position)
            .map_err(|_| FilterError::InvalidSelection)?;
        if cursor
            .columns()
            .get(projected)
            .is_none_or(|column| column.type_descriptor() != criterion.declaration)
        {
            return Err(FilterError::UnsupportedField.into());
        }
    }

    let mut ordinals = Vec::new();
    let mut source_rows = 0;
    loop {
        let batch = cursor.next_page(SCAN_BATCH_ROWS)?;
        for (index, row) in batch.rows().iter().enumerate() {
            check(cancelled)?;
            work.charge(Resource::Steps, 1)?;
            let ordinal = batch
                .offset()
                .checked_add(u32::try_from(index).map_err(|_| FilterError::InvalidSelection)?)
                .ok_or(FilterError::InvalidSelection)?;
            let mut keep = true;
            for ((criterion, &position), prepared) in
                predicates.iter().zip(&positions).zip(&prepared)
            {
                let projected = projection
                    .binary_search(&position)
                    .map_err(|_| FilterError::InvalidSelection)?;
                let value = row.get(projected).ok_or(FilterError::InvalidSelection)?;
                let policy = if criteria::needs_text_policy(criterion, value) {
                    match schema.columns()[position].text_sort_order() {
                        Some(order) => resolver.resolve_native(order)?,
                        None => resolver.default_collation(),
                    }
                } else {
                    resolver.default_collation()
                };
                // All terms execute even after an earlier false/unknown term.
                let truth = criteria::evaluate(
                    criterion,
                    value,
                    policy,
                    prepared.as_ref(),
                    work,
                    &mut pattern_work,
                    cancelled,
                )?;
                keep &= truth == Truth::True;
            }
            if keep {
                if ordinals.len() == ordinals.capacity() {
                    let additional = ordinals.capacity().max(INITIAL_MAP_CAPACITY);
                    work.charge_allocation(additional as u64, size_of::<u32>() as u64)?;
                    ordinals.try_reserve_exact(additional)?;
                }
                ordinals.push(ordinal);
            }
            source_rows = ordinal
                .checked_add(1)
                .ok_or(FilterError::InvalidSelection)?;
        }
        if batch.reached_end() {
            break;
        }
    }

    check(cancelled)?;
    Ok((ordinals, source_rows))
}
