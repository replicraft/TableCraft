//! Original-bound I32 UPDATE facts with fully checked immutable Binary owners.
use super::{CheckedIndexes, IndexTargets};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck, TableSchema, WriteBlocker,
    check_table,
    format::{
        data_type::{INT32, MAX_SHORT_BINARY_BYTES, SHORT_BINARY},
        layout::{CALCULATED_EXT_MASK, FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    table::{indexes::mutation::normal_index_audit::Ownership, mutation::preserved_source::Source},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) struct I32UpdateIndexes<'a> {
    mode: Mode<'a>,
}

enum Mode<'a> {
    Legacy(CheckedIndexes<'a>),
    Omitted(CheckedIndexes<'a>),
    Retained {
        image: &'a NativeImage,
        checked: TableCheck,
    },
}

impl<'a> I32UpdateIndexes<'a> {
    pub(in crate::table::mutation) fn capture(
        image: &'a NativeImage,
        table: &str,
        source: &Source,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        // Select using original declarations, never an error or changed-key list.
        let schema = source.table.schema();
        let mut retained = false;
        let mut omitted = false;
        for index in source.indexes.indexes() {
            omitted |= index
                .constraints()
                .is_some_and(|flags| flags.ignore_nulls());
            for key in index.columns() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, schema.columns().len() as u64)
                    .map_err(InspectError::from)?;
                let field = schema
                    .columns()
                    .iter()
                    .find(|field| field.number() == key.number())
                    .ok_or(EditError::Preservation)?;
                retained |= field.native_type() == SHORT_BINARY;
            }
        }
        if !retained && omitted {
            let facts = CheckedIndexes::capture(image, table, budget, cancelled)?;
            require_definition(schema, facts.checked().schema(), budget, cancelled)?;
            Ownership::capture_i32_omissions(image, facts.checked(), budget, cancelled)?;
            return Ok(Self {
                mode: Mode::Omitted(facts),
            });
        }
        if !retained {
            return Ok(Self {
                mode: Mode::Legacy(CheckedIndexes::capture(image, table, budget, cancelled)?),
            });
        }

        let checked = check_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        require_definition(schema, checked.schema(), budget, cancelled)?;
        for field in checked.schema().columns() {
            check(budget, cancelled)?;
            if field.scalar_type().is_none() || field.extended_flags() & CALCULATED_EXT_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        for selected in checked.indexes() {
            check(budget, cancelled)?;
            if selected.status() != IndexCheckStatus::Checked {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            let index = selected.index();
            for key in index.columns() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, checked.schema().columns().len() as u64)
                    .map_err(InspectError::from)?;
                let field = checked
                    .schema()
                    .columns()
                    .iter()
                    .find(|field| field.number() == key.number())
                    .ok_or(EditError::Preservation)?;
                match (field.native_type(), field.scalar_type()) {
                    (INT32, Some(ScalarType::Int32))
                        if field.fixed()
                            && field.storage_length() == size_of::<i32>()
                            && field.extended_flags() == 0 => {}
                    (SHORT_BINARY, Some(ScalarType::Binary)) if index.columns().len() == 1 => {
                        let flags = index.constraints().ok_or(InspectError::Unsupported)?;
                        if index.native_type() > 1
                            || index.foreign_reference().is_some()
                            || flags.ignore_nulls()
                            || flags.unique()
                            || flags.required()
                            || field.fixed()
                            || field.flags() != UPDATABLE_FLAG
                            || field.extended_flags() != 0
                            || field.complex_type_id().is_some()
                            || !(1..=MAX_SHORT_BINARY_BYTES).contains(&field.storage_length())
                        {
                            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                        }
                    }
                    _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
                }
            }
        }

        // This validates every actual tree, alias, map record and LVAL exclusion.
        // The returned token has no public mode or caller-issued permission.
        Ownership::capture_i32_updates(image, &checked, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Self {
            mode: Mode::Retained { image, checked },
        })
    }

    pub(in crate::table::mutation) fn checked(&self) -> &TableCheck {
        match &self.mode {
            Mode::Legacy(facts) | Mode::Omitted(facts) => facts.checked(),
            Mode::Retained { checked, .. } => checked,
        }
    }

    pub(in crate::table::mutation) fn legacy(&self) -> Option<&CheckedIndexes<'_>> {
        match &self.mode {
            Mode::Legacy(facts) | Mode::Omitted(facts) => Some(facts),
            Mode::Retained { .. } => None,
        }
    }

    pub(in crate::table::mutation) fn omits_nulls(&self) -> bool {
        matches!(self.mode, Mode::Omitted(_))
    }

    pub(in crate::table::mutation) fn retains_binary(&self) -> bool {
        matches!(self.mode, Mode::Retained { .. })
    }
}

impl IndexTargets for I32UpdateIndexes<'_> {
    fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match &self.mode {
            Mode::Legacy(facts) | Mode::Omitted(facts) => {
                facts.require_source(image, schema, budget, cancelled)
            }
            Mode::Retained {
                image: original,
                checked,
            } => {
                check(budget, cancelled)?;
                if !std::ptr::eq(*original, image) {
                    return Err(EditError::Preservation);
                }
                require_definition(schema, checked.schema(), budget, cancelled)
            }
        }
    }

    fn require_target(
        &self,
        schema: &TableSchema,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match &self.mode {
            Mode::Legacy(facts) | Mode::Omitted(facts) => {
                facts.require_target(schema, position, budget, cancelled)
            }
            Mode::Retained { image, .. } => {
                self.require_source(image, schema, budget, cancelled)?;
                let field = schema
                    .columns()
                    .get(position)
                    .ok_or(EditError::Preservation)?;
                if field.native_type() != INT32
                    || field.scalar_type() != Some(ScalarType::Int32)
                    || !field.fixed()
                    || field.storage_length() != size_of::<i32>()
                    || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                    || field.extended_flags() != 0
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                Ok(())
            }
        }
    }
}

fn require_definition(
    expected: &TableSchema,
    observed: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // same_definition compares both physical fields and native column vectors.
    // Pay each represented name and complete declaration envelope beforehand.
    let mut work = 0_u64;
    for schema in [expected, observed] {
        check(budget, cancelled)?;
        work = work
            .checked_add(size_of_val(schema.columns()) as u64)
            .and_then(|n| n.checked_add(size_of_val(schema.layout.fields.as_slice()) as u64))
            .and_then(|n| n.checked_add(schema.name().len() as u64))
            .ok_or(InspectError::StructureLimit)?;
        for field in &schema.layout.fields {
            check(budget, cancelled)?;
            work = work
                .checked_add(field.name.len() as u64)
                .ok_or(InspectError::StructureLimit)?;
        }
        for field in schema.columns() {
            check(budget, cancelled)?;
            work = work
                .checked_add(field.name().len() as u64)
                .and_then(|n| n.checked_add(field.field.name.len() as u64))
                .ok_or(InspectError::StructureLimit)?;
        }
    }
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    if !expected.same_definition(observed) {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
