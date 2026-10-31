use super::super::super::{
    Ace16WritePreview, EditError, WriteBlocker, gates,
    preview::{qualify, qualify_fixed},
};
use super::super::masked;
use super::FixedColumnUpdates;
use crate::table::mutation::guards::fields::physical::SourceWitness;
use crate::table::mutation::patch::spans;
use crate::{
    InspectError, NativeImage, TableCheck,
    format::data_type::NUMERIC_BYTES,
    header::check,
    limits::{MAX_COLUMNS, MAX_LIVE_ROWS},
    table::indexes::{IndexKeyUpdate, fixed_index_patches, index_patches},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Guid,
};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
enum SourceAdmission<'a> {
    Ordinary,
    BeforeChange(&'a SourceWitness),
}

pub(super) struct Table<'a> {
    pub(super) name: &'a str,
    pub(super) before: TableCheck,
    pub(super) keys: Vec<IndexKeyUpdate>,
}

impl NativeImage {
    /// Current engineering-profile column-group ceiling, not a format maximum.
    pub const MAX_FIXED_BATCH_GROUPS: usize = MAX_COLUMNS;
    /// Current cumulative assignment ceiling across all batch groups/tables.
    pub const MAX_FIXED_BATCH_ASSIGNMENTS: usize = MAX_LIVE_ROWS;

    /// Prepare one complete fixed-size scalar payload batch across tables on ACE 16.
    ///
    /// Expected values and row ordinals refer to this original image. Disjoint
    /// row-mask bits merge; every affected physical index is rewritten once with
    /// all changed components. Only the complete candidate is checked against
    /// table/relationship rules. Empty batches return exact no-ops; no candidate
    /// grants production save or journal permission. Qualified Decimal assignments
    /// reuse existing 17-byte variable slots without resizing records/offsets.
    /// Automatic cascades use a separate entry point; other variable fields,
    /// allocation and unsupported index profiles remain separate work.
    /// Atomicity here means that failed planning returns no candidate; this is
    /// not a file commit or a claim about native Access transaction semantics.
    pub fn preview_ace16_batch_updates(
        self,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        self.preview_fixed_batch_inner::<true, false>(groups, budget, cancelled)
    }

    // Fitting and allocating fixed/batch previews use extended admission;
    // UPDATE cascades retain their previous strict policy. Keep metadata
    // admission, index membership and record access together.
    pub(in crate::table::mutation::fixed) fn preview_fixed_batch_inner<
        const EXTENDED_FIXED: bool,
        const ALLOCATING: bool,
    >(
        self,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        self.preview_fixed_batch_source::<EXTENDED_FIXED, ALLOCATING>(
            groups,
            SourceAdmission::Ordinary,
            budget,
            cancelled,
        )
    }

    pub(in crate::table::mutation) fn preview_before_change_batch(
        self,
        groups: &[FixedColumnUpdates<'_>],
        witness: &SourceWitness,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        qualify(&self)?;
        self.preview_fixed_batch_source::<true, false>(
            groups,
            SourceAdmission::BeforeChange(witness),
            budget,
            cancelled,
        )
    }

    fn preview_fixed_batch_source<const EXTENDED_FIXED: bool, const ALLOCATING: bool>(
        self,
        groups: &[FixedColumnUpdates<'_>],
        source: SourceAdmission<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span =
            tracing::debug_span!("native.edit.batch_preview", groups = groups.len()).entered();
        check(budget, cancelled)?;
        if EXTENDED_FIXED && !ALLOCATING {
            qualify_fixed(&self)?;
        } else {
            qualify(&self)?;
        }
        if groups.len() > Self::MAX_FIXED_BATCH_GROUPS {
            return Err(InspectError::StructureLimit.into());
        }
        if matches!(source, SourceAdmission::Ordinary)
            && EXTENDED_FIXED
            && !ALLOCATING
            && super::super::preserved::selected(&self, groups, budget, cancelled)?
        {
            return super::super::preserved::preview(self, groups, budget, cancelled);
        }
        let mut count = 0_usize;
        let mut tables: Vec<Table<'_>> = Vec::new();
        budget
            .charge_allocation(groups.len() as u64, size_of::<Table<'_>>() as u64)
            .map_err(InspectError::from)?;
        tables
            .try_reserve_exact(groups.len())
            .map_err(|_| InspectError::Allocation)?;
        for (position, group) in groups.iter().enumerate() {
            check(budget, cancelled)?;
            count = count
                .checked_add(group.updates().len())
                .filter(|count| *count <= Self::MAX_FIXED_BATCH_ASSIGNMENTS)
                .ok_or(InspectError::StructureLimit)?;
            budget
                .charge(Resource::Steps, position as u64)
                .map_err(InspectError::from)?;
            if groups[..position].iter().any(|prior| {
                prior.table().eq_ignore_ascii_case(group.table())
                    && prior.column().eq_ignore_ascii_case(group.column())
            }) {
                return Err(EditError::Conflict);
            }
            if !tables
                .iter()
                .any(|table| table.name.eq_ignore_ascii_case(group.table()))
            {
                let before = match source {
                    SourceAdmission::BeforeChange(witness) => gates::before_prepared_fields(
                        &self,
                        group.table(),
                        witness,
                        budget,
                        cancelled,
                    )?,
                    SourceAdmission::Ordinary if EXTENDED_FIXED => {
                        gates::before_fixed_update(&self, group.table(), budget, cancelled)?
                    }
                    SourceAdmission::Ordinary => {
                        gates::before(&self, group.table(), budget, cancelled)?
                    }
                };
                if !ALLOCATING {
                    super::super::super::data_ownership::require_index_disjointness(
                        &self,
                        before.schema(),
                        budget,
                        cancelled,
                    )?;
                }
                if self.fixed_update_limits().is_some() {
                    // Pay comparisons before the borrowed filter. Original groups
                    // determine this table's complete affected-column set once.
                    for candidate in groups {
                        check(budget, cancelled)?;
                        budget
                            .charge(
                                Resource::Steps,
                                candidate.table().len().max(group.table().len()) as u64,
                            )
                            .map_err(InspectError::from)?;
                    }
                    let columns = groups
                        .iter()
                        .filter(|candidate| candidate.table().eq_ignore_ascii_case(group.table()))
                        .map(|candidate| candidate.column());
                    super::super::extended::qualify(&self, &before, columns, budget, cancelled)?;
                }
                tables.push(Table {
                    name: group.table(),
                    before,
                    keys: Vec::new(),
                });
            }
        }
        budget
            .charge_allocation(count as u64, size_of::<(u32, u32)>() as u64)
            .map_err(InspectError::from)?;
        let mut changed = Vec::new();
        changed
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        let mut patches = Vec::new();
        for group in groups {
            check(budget, cancelled)?;
            let table = tables
                .iter_mut()
                .find(|table| table.name.eq_ignore_ascii_case(group.table()))
                .ok_or(InspectError::Malformed)?;
            let kind = table
                .before
                .schema()
                .columns()
                .iter()
                .find(|column| column.name().eq_ignore_ascii_case(group.column()))
                .and_then(|column| column.scalar_type())
                .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
            if ALLOCATING && kind != ScalarType::Int32 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            let mut target = Target::<EXTENDED_FIXED> {
                table,
                patches: &mut patches,
                changed: &mut changed,
            };
            match kind {
                ScalarType::Boolean => target.stage::<0>(&self, group, kind, budget, cancelled)?,
                ScalarType::Byte => {
                    target.stage::<{ size_of::<u8>() }>(&self, group, kind, budget, cancelled)?
                }
                ScalarType::Int16 => {
                    target.stage::<{ size_of::<i16>() }>(&self, group, kind, budget, cancelled)?
                }
                ScalarType::Int32 | ScalarType::Single => {
                    target.stage::<{ size_of::<i32>() }>(&self, group, kind, budget, cancelled)?
                }
                ScalarType::Int64
                | ScalarType::Currency
                | ScalarType::Double
                | ScalarType::DateTime => {
                    target.stage::<{ size_of::<i64>() }>(&self, group, kind, budget, cancelled)?
                }
                ScalarType::Decimal => {
                    target.stage::<NUMERIC_BYTES>(&self, group, kind, budget, cancelled)?
                }
                ScalarType::Guid => {
                    target.stage::<{ Guid::BYTE_LEN }>(&self, group, kind, budget, cancelled)?
                }
                _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
            }
        }
        spans::coalesce(&mut patches, budget, cancelled)?;
        if ALLOCATING && !tables.is_empty() {
            unique_changed(&mut changed, budget)?;
            return super::split::finish(
                self,
                super::split::Operation {
                    tables: &tables,
                    groups,
                    fields: &patches,
                    changed_rows: changed.len(),
                },
                budget,
                cancelled,
            );
        }
        let mut indexes = Vec::new();
        for table in &tables {
            check(budget, cancelled)?;
            let staged = if EXTENDED_FIXED {
                fixed_index_patches(
                    &self,
                    table.before.schema(),
                    table.before.indexes(),
                    &table.keys,
                    budget,
                    cancelled,
                )?
            } else {
                index_patches(
                    &self,
                    table.before.schema(),
                    table.before.indexes(),
                    &table.keys,
                    budget,
                    cancelled,
                )?
            };
            budget
                .charge_allocation(
                    staged.len() as u64,
                    size_of::<crate::table::indexes::IndexDataPatch>() as u64,
                )
                .map_err(InspectError::from)?;
            indexes
                .try_reserve(staged.len())
                .map_err(|_| InspectError::Allocation)?;
            indexes.extend(staged);
        }
        unique_changed(&mut changed, budget)?;
        check(budget, cancelled)?;
        budget
            .charge_allocation(tables.len() as u64, size_of::<&str>() as u64)
            .map_err(InspectError::from)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(tables.len())
            .map_err(|_| InspectError::Allocation)?;
        names.extend(tables.iter().map(|table| table.name));
        self.finish_tables_updates(&names, &patches, &indexes, changed.len(), budget, cancelled)
            .map(Ace16WritePreview::from_plan)
    }
}

struct Target<'a, 'b, const EXTENDED_FIXED: bool> {
    table: &'a mut Table<'b>,
    patches: &'a mut Vec<spans::Patch>,
    changed: &'a mut Vec<(u32, u32)>,
}
impl<const EXTENDED_FIXED: bool> Target<'_, '_, EXTENDED_FIXED> {
    fn stage<const WIDTH: usize>(
        &mut self,
        image: &NativeImage,
        group: &FixedColumnUpdates<'_>,
        kind: ScalarType,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let column = masked::stage::<WIDTH, EXTENDED_FIXED>(
            image,
            &self.table.before,
            group.column(),
            group.updates(),
            kind,
            budget,
            cancelled,
        )?;
        for patch in &column.patches {
            spans::push(self.patches, patch, budget, cancelled)?;
        }
        budget
            .charge_allocation(column.keys.len() as u64, size_of::<IndexKeyUpdate>() as u64)
            .map_err(InspectError::from)?;
        self.table
            .keys
            .try_reserve(column.keys.len())
            .map_err(|_| InspectError::Allocation)?;
        self.table.keys.extend(column.keys);
        self.changed.extend(
            column
                .changed
                .into_iter()
                .map(|reference| (self.table.before.schema().definition_page(), reference)),
        );
        Ok(())
    }
}

fn unique_changed(
    changed: &mut Vec<(u32, u32)>,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    let levels = changed.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(Resource::Steps, (changed.len() * levels) as u64)
        .map_err(InspectError::from)?;
    changed.sort_unstable();
    changed.dedup();
    Ok(())
}
