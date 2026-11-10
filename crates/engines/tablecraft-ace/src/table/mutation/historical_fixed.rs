//! Existing unindexed LONG changes in the closed current-schema source family.
use super::{
    current_schema,
    fixed_records::{self, proof, stage},
    no_required_programs::NoRequiredTablePrograms,
    patch::BytePatch,
    preserved_source,
};
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{live_row_range, row_count},
        row::OFFSET_MASK,
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const SOURCE_PAGES: usize = 11;
const USER_DEFINITION: u32 = 7;
const USER_DATA: usize = 8;

/// This proof has a separate purpose from ordinary and readonly schema sources.
/// Its only write consumer requires already present unindexed LONG targets.
pub(in crate::table::mutation) struct Source<'image> {
    image: &'image NativeImage,
    facts: preserved_source::Source,
}

impl<'image> Source<'image> {
    fn capture(
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let current = current_schema::observe(image, table, budget, cancelled).map_err(
            |error| match error {
                current_schema::Error::Inspect(error) => EditError::Inspect(error),
                current_schema::Error::Native(error) => error,
                current_schema::Error::Source => EditError::Unsupported(WriteBlocker::RowLayout),
            },
        )?;
        let certificate = NoRequiredTablePrograms::capture_historical_present_fixed(
            image, &current, budget, cancelled,
        )?;
        let facts = preserved_source::capture_historical_present_fixed(
            image,
            table,
            &certificate,
            budget,
            cancelled,
        )?;
        if !facts.dependencies.is_empty() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        Ok(Self { image, facts })
    }

    pub(super) fn facts(&self) -> &preserved_source::Source {
        &self.facts
    }

    pub(super) fn require_image(&self, image: &NativeImage) -> Result<(), EditError> {
        if !std::ptr::eq(self.image, image) {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    fn qualify(
        &self,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let schema = self.facts.table.schema();
        if groups.is_empty() {
            return Err(EditError::Conflict);
        }
        fixed_records::binding::all_normal(&self.facts, groups, budget, cancelled)?;
        for group in groups {
            check(budget, cancelled)?;
            let field = schema
                .columns()
                .iter()
                .find(|field| field.name().eq_ignore_ascii_case(group.column()))
                .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
            if field.native_type() != INT32
                || !field.fixed()
                || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || field.extended_flags() != 0
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            for index in self.facts.indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    if key.number() == field.number() {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                }
            }
            for update in group.updates() {
                check(budget, cancelled)?;
                let record = self
                    .facts
                    .records
                    .get(update.row())
                    .ok_or(EditError::Conflict)?;
                let layout = record_layout::parse(&record.data, schema, budget)?;
                if field.number() >= layout.count {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                let end = crate::format::row::COLUMN_COUNT_BYTES
                    .checked_add(field.field.offset)
                    .and_then(|offset| offset.checked_add(size_of::<i32>()))
                    .ok_or(EditError::Preservation)?;
                if end > layout.data_end {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
        }
        Ok(())
    }
}

impl NativeImage {
    pub(crate) fn plan_historical_present_fixed_updates(
        self,
        table: &str,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let source = Source::capture(&self, table, budget, cancelled)?;
        source.qualify(groups, budget, cancelled)?;
        let (patches, changed_rows) =
            stage::prepare_historical_present(&self, &source, groups, budget, cancelled)?;
        let mut after = super::record_serialization::copy(self.as_bytes(), budget, cancelled)?;
        for patch in patches {
            check(budget, cancelled)?;
            let end = patch
                .offset()
                .checked_add(patch.before().len())
                .ok_or(EditError::Preservation)?;
            let bytes = after
                .get_mut(patch.offset()..end)
                .ok_or(EditError::Preservation)?;
            if bytes != patch.before() {
                return Err(EditError::Preservation);
            }
            budget
                .charge(Resource::Steps, bytes.len() as u64)
                .map_err(InspectError::from)?;
            bytes.copy_from_slice(patch.after());
        }
        let original = super::record_serialization::copy(self.as_bytes(), budget, cancelled)?;
        proof::prove_original(
            proof::Original {
                image: &self,
                source: source.facts(),
            },
            original,
            groups,
            proof::Observed {
                bytes: &after,
                changed_rows,
            },
            budget,
            cancelled,
        )?;
        drop(source);
        Ok(NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows,
        })
    }
}

impl NativeImage {
    /// Select one historical-record attempt from inert current metadata/headers.
    /// This grants no permission and never retries failed current-source admission.
    pub fn select_native_historical_present_fixed_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        if self.catalog().header().format() != crate::FileFormat::Ace2016
            || self.catalog().header().subversion() != 0
            || self.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
        {
            return Ok(false);
        }
        let schema = crate::inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        if schema.definition_page() != USER_DEFINITION {
            return Ok(false);
        }
        // Only this closed candidate geometry is selected. Complete usage-map,
        // role, root, program and property admission still belongs to Source.
        let page = &self.as_bytes()[USER_DATA * PAGE_BYTES..(USER_DATA + 1) * PAGE_BYTES];
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(page, OWNER_OFFSET)? != USER_DEFINITION
        {
            return Ok(false);
        }
        for slot in 0..row_count(page)? {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            let Some((flags, start, end)) = live_row_range(page, slot)? else {
                continue;
            };
            if flags & !OFFSET_MASK != 0 {
                continue;
            }
            let count = usize::from(u16_at(&page[start..end], 0)?);
            if count > 0 && count < schema.layout.max_columns {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Fully qualify the separate current-source purpose and at least one
    /// already-present unindexed LONG target without preparing assignments.
    pub fn check_native_historical_present_fixed_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Source::capture(self, table, budget, cancelled)?;
        for field in source.facts.table.schema().columns() {
            check(budget, cancelled)?;
            if field.native_type() != INT32
                || !field.fixed()
                || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || field.extended_flags() != 0
            {
                continue;
            }
            let mut indexed = false;
            for index in source.facts.indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    indexed |= key.number() == field.number();
                }
            }
            if indexed {
                continue;
            }
            let groups = [FixedColumnUpdates::new(table, field.name(), &[])];
            source.qualify(&groups, budget, cancelled)?;
            return Ok(());
        }
        Err(EditError::Unsupported(WriteBlocker::Column))
    }
}
