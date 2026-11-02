//! Retire only complete selected LVAL pages and rebuild each fitted parent once.
use super::{Page, Retiring, authorize, replacement};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    check_table_long_values,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::{
        long_value::trace::{self, Kind},
        mutation::{
            map_changes::{ByteEdit, merge},
            normal_fitted::{
                binding::{self, Bound, Target},
                original_values,
                stage::Patch,
            },
            preserved_source::Source,
            record_serialization::{self, policy::RecordPolicy, variables},
        },
        record_layout,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const INVALID_PREFIX_BYTES: usize = size_of::<u32>();

pub(in crate::table::mutation::normal_fitted::external) struct Prepared {
    values: Vec<Retiring>,
    pages: Vec<Page>,
    rows: usize,
}

pub(in crate::table::mutation::normal_fitted::external) fn capture(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    let schema = source.table.schema();
    let mut retiring = binding::vector(bound.assignments.len(), budget)?;
    let mut rows = 0_usize;
    for selected in &bound.rows {
        check(budget, cancelled)?;
        let original = &source.records[selected.ordinal];
        let values = original_values(
            image,
            source,
            selected.ordinal,
            &bound.columns,
            budget,
            cancelled,
        )?;
        let layout = record_layout::parse(&original.data, schema, budget)?;
        let mut changed = false;
        for assignment in &bound.assignments[selected.start..selected.end] {
            check(budget, cancelled)?;
            let cell = groups[assignment.group]
                .cell(assignment.index)
                .ok_or(EditError::Preservation)?;
            let actual = binding::value(bound, &values, assignment.group, budget, cancelled)?;
            if !binding::equal(actual, cell.expected, budget, cancelled)? {
                return Err(EditError::Conflict);
            }
            if binding::equal(cell.expected, cell.value, budget, cancelled)? {
                continue;
            }
            let Target::Variable { column } = bound.targets[assignment.group] else {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            };
            let field = &schema.columns()[column];
            if !field.is_ordinary_long_value() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            // Mixed retained-descriptor changes require a separately qualified
            // parent-composition law; this branch never retries another planner.
            let replacement =
                replacement(cell.value).ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            let from = *layout
                .offsets
                .get(field.field.variable)
                .ok_or(EditError::Preservation)?;
            let to = *layout
                .offsets
                .get(field.field.variable + 1)
                .ok_or(EditError::Preservation)?;
            let raw = original.data.get(from..to).ok_or(EditError::Preservation)?;
            if raw.len() != DESCRIPTOR_BYTES || raw[2 * size_of::<u32>()..] != [0; size_of::<u32>()]
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let mut bytes = image.reader();
            let mut reader = Reader {
                source: &mut bytes,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            };
            let traced = trace::read(&mut reader, raw)?;
            if !matches!(traced.kind, Kind::Single | Kind::Chained) || traced.chunks.is_empty() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let mut references = binding::vector(traced.chunks.len(), reader.budget)?;
            for chunk in traced.chunks {
                reader.tick()?;
                references.push(chunk.reference);
            }
            retiring.push(Retiring {
                ordinal: selected.ordinal,
                column,
                references,
                replacement,
            });
            changed = true;
        }
        rows = rows
            .checked_add(usize::from(changed))
            .ok_or(InspectError::StructureLimit)?;
    }
    let pages = authorize(image, source, &retiring, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(Prepared {
        values: retiring,
        pages,
        rows,
    })
}

fn parents(
    source: &Source,
    values: &[Retiring],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Patch>, EditError> {
    let mut patches: Vec<Patch> = binding::vector(values.len(), budget)?;
    for value in values {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, patches.len() as u64)
            .map_err(InspectError::from)?;
        if patches
            .iter()
            .any(|patch| patch.offset == source.records[value.ordinal].offset)
        {
            continue;
        }
        let original = &source.records[value.ordinal];
        let frame = record_layout::parse(&original.data, source.table.schema(), budget)?;
        let mut cells = binding::vector(values.len(), budget)?;
        for selected in values {
            check(budget, cancelled)?;
            if selected.ordinal == value.ordinal {
                cells.push(variables::Cell {
                    column: selected.column,
                    value: &selected.replacement,
                });
            }
        }
        let after = variables::rebuild(
            source.table.schema(),
            &original.data,
            &frame,
            &cells,
            RecordPolicy::FittedDeclared,
            budget,
            cancelled,
        )?;
        if after.len() != original.data.len() {
            return Err(EditError::Preservation);
        }
        patches.push(Patch {
            offset: original.offset,
            before: record_serialization::copy(&original.data, budget, cancelled)?,
            after,
        });
    }
    Ok(patches)
}

fn pages_and_maps(
    image: &NativeImage,
    source: &Source,
    pages: &[Page],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Patch>, EditError> {
    if pages.is_empty() {
        return Ok(Vec::new());
    }
    let storage = check_table_long_values(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        source.table.schema().name(),
        budget,
        cancelled,
    )?;
    let capacity = pages
        .len()
        .checked_mul(3)
        .ok_or(InspectError::StructureLimit)?;
    // Preallocation pays the complete capacity; shared merge never reallocates.
    let mut edits: Vec<ByteEdit> = binding::vector(capacity, budget)?;
    let patch_count = capacity
        .checked_add(pages.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut patches = binding::vector(patch_count, budget)?;
    let mut bytes = image.reader();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    for page in pages {
        reader.tick()?;
        let field = &source.table.schema().columns()[page.column];
        let mut maps = None;
        for report in storage.columns() {
            reader.tick()?;
            if report.column_number() == field.number() {
                maps = report.maps();
            }
        }
        let maps = maps.ok_or(EditError::Preservation)?;
        let owned = RepresentedMap::read(&mut reader, maps.owned(), MapRole::Owned)?;
        let hint =
            RepresentedMap::read(&mut reader, maps.candidate(), MapRole::LongValueCandidate)?;
        let owned_cell = owned.cell(&mut reader, page.number)?;
        merge(&mut edits, owned_cell, false, &mut reader)?;
        let candidate = hint.cell(&mut reader, page.number)?;
        if candidate.before & candidate.mask != 0 {
            merge(&mut edits, candidate, false, &mut reader)?;
        }
        let free_cell = global.cell(&mut reader, page.number)?;
        merge(&mut edits, free_cell, true, &mut reader)?;
        let offset = page.number as usize * PAGE_BYTES;
        patches.push(Patch {
            offset,
            before: record_serialization::copy(
                &image.as_bytes()[offset..offset + INVALID_PREFIX_BYTES],
                reader.budget,
                cancelled,
            )?,
            after: record_serialization::copy(
                &[0; INVALID_PREFIX_BYTES],
                reader.budget,
                cancelled,
            )?,
        });
    }
    for edit in edits {
        reader.tick()?;
        patches.push(Patch {
            offset: edit.offset,
            before: record_serialization::copy(&[edit.before], reader.budget, cancelled)?,
            after: record_serialization::copy(
                &[(edit.before | edit.set) & !edit.clear],
                reader.budget,
                cancelled,
            )?,
        });
    }
    reader.tick()?;
    Ok(patches)
}

pub(in crate::table::mutation::normal_fitted::external) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Patch>, usize), EditError> {
    let prepared = capture(image, source, groups, bound, budget, cancelled)?;
    let mut patches = parents(source, &prepared.values, budget, cancelled)?;
    let retirement = pages_and_maps(image, source, &prepared.pages, budget, cancelled)?;
    let count = patches
        .len()
        .checked_add(retirement.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut combined = binding::vector(count, budget)?;
    for patch in patches.drain(..) {
        check(budget, cancelled)?;
        combined.push(patch);
    }
    for patch in retirement {
        check(budget, cancelled)?;
        combined.push(patch);
    }
    check(budget, cancelled)?;
    Ok((combined, prepared.rows))
}
