//! Current sparse declarations use one source-bound scalar replacement language.
mod proof;

use super::{
    field, fixed_records,
    historical_fixed::source::Source,
    normal_fitted::binding,
    record_insertion::payload as insertion_payload,
    record_placement::{audit, forward::Plan, owned::Reservations, space::Space},
    record_roots, record_serialization,
    rows::{self, RowAssignment},
};
use crate::{
    EditError, FittedColumnUpdates, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage,
    WriteBlocker,
    format::{
        data_type::{BOOLEAN, BOOLEAN_BYTES, CURRENCY, GUID, INT32, NUMERIC, SHORT_TEXT},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        row::{OFFSET_MASK, RELOCATED_MASK},
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const SOURCE_PAGES: usize = 11;
const USER_DEFINITION: u32 = 7;

#[derive(Clone, Copy)]
pub(super) enum Family {
    Current,
    Retained,
    RetainedOpaque,
    RetainedCurrency,
    RetainedBoolean,
    RetainedBooleanMaterialization,
    RetainedFixedScalars,
    RetainedWideValues,
}

impl Family {
    pub(super) fn capture<'image>(
        self,
        image: &'image NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Source<'image>, EditError> {
        match self {
            Self::Current => Source::capture_current_rows(image, table, budget, cancelled),
            Self::RetainedOpaque => {
                if !image.has_retained_table_source(table, budget, cancelled)? {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                let source = Source::capture_retained_opaque_rows(image, table, budget, cancelled)?;
                if !super::retained_opaque_row_updates::source_has_ambiguous(
                    &source, budget, cancelled,
                )? {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                Ok(source)
            }
            Self::Retained
            | Self::RetainedCurrency
            | Self::RetainedBoolean
            | Self::RetainedBooleanMaterialization
            | Self::RetainedFixedScalars
            | Self::RetainedWideValues => {
                if !image.has_retained_table_source(table, budget, cancelled)? {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                Source::capture_retained_rows(image, table, budget, cancelled)
            }
        }
    }

    pub(super) fn copy_image(
        self,
        image: &NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        match self {
            Self::Current => record_serialization::copy(image.as_bytes(), budget, cancelled),
            Self::Retained
            | Self::RetainedOpaque
            | Self::RetainedCurrency
            | Self::RetainedBoolean
            | Self::RetainedBooleanMaterialization
            | Self::RetainedFixedScalars
            | Self::RetainedWideValues => fixed_records::copy(image.as_bytes(), budget, cancelled),
        }
    }

    pub(super) fn roots<'u, U: RowAssignment>(
        self,
        image: &NativeImage,
        schema: &crate::TableSchema,
        requests: &'u [U],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<record_roots::Roots<'u, U>, EditError> {
        match self {
            Self::Current => record_roots::capture(image, schema, requests, budget, cancelled),
            Self::Retained
            | Self::RetainedOpaque
            | Self::RetainedCurrency
            | Self::RetainedBoolean
            | Self::RetainedBooleanMaterialization
            | Self::RetainedFixedScalars
            | Self::RetainedWideValues => {
                record_roots::capture_retained(image, schema, requests, budget, cancelled)
            }
        }
    }

    pub(super) fn space(
        self,
        image: &NativeImage,
        schema: &crate::TableSchema,
        owned: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Space, EditError> {
        match self {
            Self::Current => Space::capture_normal(image, schema, owned, budget, cancelled),
            Self::Retained
            | Self::RetainedOpaque
            | Self::RetainedCurrency
            | Self::RetainedBoolean
            | Self::RetainedBooleanMaterialization
            | Self::RetainedFixedScalars
            | Self::RetainedWideValues => {
                Space::capture_retained(image, schema, owned, budget, cancelled)
            }
        }
    }

    fn language(self) -> Result<record_serialization::current_rows::Language, EditError> {
        match self {
            Self::RetainedCurrency => Ok(record_serialization::current_rows::Language::Currency),
            Self::RetainedFixedScalars => {
                Ok(record_serialization::current_rows::Language::FixedScalars)
            }
            Self::RetainedWideValues => {
                Ok(record_serialization::current_rows::Language::WideValues)
            }
            Self::Current | Self::Retained | Self::RetainedOpaque => {
                Ok(record_serialization::current_rows::Language::LongText)
            }
            Self::RetainedBooleanMaterialization => {
                Ok(record_serialization::current_rows::Language::BooleanMaterialization)
            }
            Self::RetainedBoolean => Err(EditError::Preservation),
        }
    }

    pub(super) fn qualify_record(
        self,
        schema: &crate::TableSchema,
        column: usize,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if matches!(self, Self::RetainedBoolean) {
            record_serialization::boolean_rows::qualify(schema, column, before, budget, cancelled)?;
            return Ok(());
        }
        if matches!(self, Self::RetainedOpaque) {
            if schema
                .columns()
                .get(column)
                .is_none_or(|field| field.native_type() != INT32)
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            if record_serialization::opaque_rows::is_ambiguous(schema, before, budget, cancelled)? {
                return record_serialization::opaque_rows::qualify(
                    schema, column, before, budget, cancelled,
                );
            }
        }
        record_serialization::current_rows::qualify(
            schema,
            column,
            before,
            self.language()?,
            budget,
            cancelled,
        )?;
        Ok(())
    }

    fn build_record(
        self,
        schema: &crate::TableSchema,
        column: usize,
        before: &[u8],
        value: &Value,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        if matches!(self, Self::RetainedBoolean) {
            return record_serialization::boolean_rows::build(
                schema, column, before, value, budget, cancelled,
            );
        }
        if matches!(self, Self::RetainedOpaque)
            && record_serialization::opaque_rows::is_ambiguous(schema, before, budget, cancelled)?
        {
            record_serialization::opaque_rows::build(
                schema, column, before, value, budget, cancelled,
            )
        } else {
            record_serialization::current_rows::build(
                schema,
                column,
                before,
                value,
                self.language()?,
                budget,
                cancelled,
            )
        }
    }

    pub(super) fn reconstruct_record(
        self,
        schema: &crate::TableSchema,
        column: usize,
        before: &[u8],
        value: &Value,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        if matches!(self, Self::RetainedBoolean) {
            return record_serialization::proof::boolean_rows::reconstruct(
                schema, column, before, value, budget, cancelled,
            );
        }
        if matches!(self, Self::RetainedOpaque)
            && record_serialization::opaque_rows::is_ambiguous(schema, before, budget, cancelled)?
        {
            record_serialization::proof::opaque_rows::reconstruct(
                schema, column, before, value, budget, cancelled,
            )
        } else {
            record_serialization::proof::current_rows::reconstruct(
                schema,
                column,
                before,
                value,
                self.language()?,
                budget,
                cancelled,
            )
        }
    }
}

pub(super) struct Selected {
    ordinal: usize,
    column: usize,
    changed: bool,
}

impl RowAssignment for Selected {
    fn row(&self) -> usize {
        self.ordinal
    }
}

pub(super) fn bind_for(
    image: &NativeImage,
    source: &Source<'_>,
    groups: &[FittedColumnUpdates<'_>],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Selected, EditError> {
    if groups.len() != 1 || groups[0].len() != 1 {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let schema = source.facts().table.schema();
    let column = if matches!(
        family,
        Family::Retained
            | Family::RetainedOpaque
            | Family::RetainedCurrency
            | Family::RetainedBoolean
            | Family::RetainedBooleanMaterialization
            | Family::RetainedFixedScalars
            | Family::RetainedWideValues
    ) {
        retained_column(source, &groups[0], family, budget, cancelled)?
    } else {
        match &groups[0] {
            FittedColumnUpdates::Fixed { column, updates } => {
                let requests = [FixedColumnUpdates::new(schema.name(), column, updates)];
                let (bound, _) = fixed_records::binding::all_normal(
                    source.facts(),
                    &requests,
                    budget,
                    cancelled,
                )?;
                bound.first().ok_or(EditError::Preservation)?.column
            }
            FittedColumnUpdates::Variable { .. } => {
                let bound = binding::all(source.facts(), groups, budget, cancelled)?;
                *bound.columns.first().ok_or(EditError::Preservation)?
            }
        }
    };
    for index in source.facts().indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == schema.columns()[column].number() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }
    let update = groups[0].cell(0).ok_or(EditError::Preservation)?;
    match (schema.columns()[column].native_type(), update.value) {
        (INT32, Value::Int32(_) | Value::Null)
            if matches!(groups[0], FittedColumnUpdates::Fixed { .. }) => {}
        (SHORT_TEXT, Value::Text(_) | Value::Null)
            if matches!(groups[0], FittedColumnUpdates::Variable { .. }) => {}
        (BOOLEAN, Value::Boolean(_))
            if matches!(
                family,
                Family::RetainedBoolean | Family::RetainedBooleanMaterialization
            ) && matches!(groups[0], FittedColumnUpdates::Fixed { .. }) => {}
        (CURRENCY, Value::Currency(_) | Value::Null)
            if matches!(family, Family::RetainedCurrency)
                && matches!(groups[0], FittedColumnUpdates::Fixed { .. }) => {}
        (GUID, Value::Guid(_) | Value::Null)
            if matches!(family, Family::RetainedWideValues)
                && matches!(groups[0], FittedColumnUpdates::Fixed { .. }) => {}
        (NUMERIC, Value::Decimal(_) | Value::Null)
            if matches!(family, Family::RetainedWideValues)
                && matches!(groups[0], FittedColumnUpdates::Variable { .. }) => {}
        (native, value)
            if matches!(family, Family::RetainedFixedScalars)
                && matches!(groups[0], FittedColumnUpdates::Fixed { .. })
                && record_serialization::current_rows::fixed_scalar_value(native, value) => {}
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    let values = super::normal_fitted::original_values(
        image,
        source.facts(),
        update.row,
        &[column],
        budget,
        cancelled,
    )?;
    let original = values.first().ok_or(EditError::Preservation)?;
    if !binding::equal(original, update.expected, budget, cancelled)? {
        return Err(EditError::Conflict);
    }
    let changed = !binding::equal(original, update.value, budget, cancelled)?;
    let record = source
        .facts()
        .records
        .get(update.row)
        .ok_or(EditError::Conflict)?;
    family.qualify_record(schema, column, &record.data, budget, cancelled)?;
    Ok(Selected {
        ordinal: update.row,
        column,
        changed,
    })
}

impl NativeImage {
    /// Original declaration history selects the current-row source before proof.
    pub fn select_native_current_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        let header = self.catalog().header();
        if header.format() != crate::FileFormat::Ace2016
            || header.subversion() != 0
            || header.encoded_pages()
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
        Ok(schema.definition_page() == USER_DEFINITION
            && schema.physical_layout().max_columns > schema.columns().len())
    }

    /// Prove actual current programs, constraints, ownership and record framing
    /// before presenting an eligible unindexed LONG or Short Text editor.
    pub fn check_native_current_rows_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = Source::capture_current_rows(self, table, budget, cancelled)?;
        let schema = source.facts().table.schema();
        for (column, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let supported = (field.native_type() == INT32
                && field.fixed()
                && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0)
                || (field.native_type() == SHORT_TEXT && !field.fixed());
            if !supported {
                continue;
            }
            let mut indexed = false;
            for index in source.facts().indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    indexed |= key.number() == field.number();
                }
            }
            if indexed {
                continue;
            }
            if field.fixed() {
                let groups = [FixedColumnUpdates::new(table, field.name(), &[])];
                fixed_records::binding::all_normal(source.facts(), &groups, budget, cancelled)?;
            } else {
                let groups = [FittedColumnUpdates::Variable {
                    column: field.name(),
                    updates: &[],
                }];
                binding::all(source.facts(), &groups, budget, cancelled)?;
            }
            for record in &source.facts().records {
                record_serialization::current_rows::qualify(
                    schema,
                    column,
                    &record.data,
                    record_serialization::current_rows::Language::LongText,
                    budget,
                    cancelled,
                )?;
            }
            return Ok(());
        }
        Err(EditError::Unsupported(WriteBlocker::Column))
    }

    /// Requests retain the same metadata-selected language, without retrying
    /// a failed historical source under a more permissive interpretation.
    pub(crate) fn current_row_updates_selected(
        &self,
        table: &str,
        _groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        self.select_native_current_rows_table(table, budget, cancelled)
    }

    pub(crate) fn plan_current_row_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        plan(self, table, groups, Family::Current, budget, cancelled)
    }
}

pub(super) fn plan(
    image: NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.current_row_updates.plan").entered();
    let source = family.capture(&image, table, budget, cancelled)?;
    let requests = [bind_for(
        &image, &source, groups, family, budget, cancelled,
    )?];
    let schema = source.facts().table.schema();
    let captured = family.roots(&image, schema, &requests, budget, cancelled)?;
    let root = captured.selected.first().ok_or(EditError::Conflict)?;
    if captured.selected.len() != 1 || !matches!(root.word & !OFFSET_MASK, 0 | RELOCATED_MASK) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let original = source
        .facts()
        .records
        .get(root.update.ordinal)
        .ok_or(EditError::Conflict)?;
    if original.offset != root.final_record.offset || original.data != root.final_record.data {
        return Err(EditError::Preservation);
    }
    let value = groups[0].cell(0).ok_or(EditError::Preservation)?.value;
    let changed = root.update.changed;
    let record = if changed {
        family.build_record(
            schema,
            root.update.column,
            &original.data,
            value,
            budget,
            cancelled,
        )?
    } else {
        record_serialization::copy(&original.data, budget, cancelled)?
    };
    let mut after = family.copy_image(&image, budget, cancelled)?;
    if changed {
        let space = family.space(&image, schema, &captured.owned, budget, cancelled)?;
        let mut reservations =
            Reservations::capture(&image, &space, &captured.owned, budget, cancelled)?;
        let reservation = reservations.reserve(&image, root, record.len(), budget, cancelled)?;
        let mut placement = Plan::new();
        placement.apply_reserved(
            &image,
            root,
            &record,
            &space,
            &reservation,
            budget,
            cancelled,
        )?;
        audit::pages(&placement, budget, cancelled)?;
        for page in &placement.pages {
            check(budget, cancelled)?;
            let at = usize::try_from(page.number)
                .ok()
                .and_then(|number| number.checked_mul(PAGE_BYTES))
                .ok_or(EditError::Preservation)?;
            let target = after
                .get_mut(at..at + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            if target != page.before.as_slice() || page.after.len() != PAGE_BYTES {
                return Err(EditError::Preservation);
            }
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            target.copy_from_slice(&page.after);
        }
        audit::final_pages(&placement, &after, budget, cancelled)?;
        space.audit(image.as_bytes(), &after, budget, cancelled)?;
    }
    proof::prove(&image, table, groups, &after, family, budget, cancelled)?;
    drop(source);
    Ok(NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows: usize::from(changed),
    })
}

fn retained_column(
    source: &Source<'_>,
    group: &FittedColumnUpdates<'_>,
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let schema = source.facts().table.schema();
    let mut selected = None;
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        fixed_records::name_work(column.name(), group.name(), budget)?;
        if column.name().eq_ignore_ascii_case(group.name()) && selected.replace(position).is_some()
        {
            return Err(EditError::Preservation);
        }
    }
    let column = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns()[column];
    match group {
        FittedColumnUpdates::Fixed { updates, .. } => {
            let (kind, width) = match family {
                Family::RetainedCurrency => (ScalarType::Currency, size_of::<i64>()),
                Family::RetainedBoolean | Family::RetainedBooleanMaterialization => {
                    (ScalarType::Boolean, BOOLEAN_BYTES)
                }
                Family::RetainedWideValues => (ScalarType::Guid, size_of::<u128>()),
                Family::RetainedFixedScalars => {
                    record_serialization::current_rows::fixed_scalar_kind(field.native_type())
                        .ok_or(EditError::Unsupported(WriteBlocker::Column))?
                }
                _ => (ScalarType::Int32, size_of::<i32>()),
            };
            field::fixed_column_at(schema, column, kind, width)?;
            rows::unique_identities(updates, budget, cancelled)?;
        }
        FittedColumnUpdates::Variable { updates, .. } => {
            if matches!(
                family,
                Family::RetainedCurrency
                    | Family::RetainedBoolean
                    | Family::RetainedBooleanMaterialization
                    | Family::RetainedFixedScalars
            ) {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            if matches!(family, Family::RetainedWideValues) {
                if field.native_type() != NUMERIC {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                insertion_payload::qualify(field)?;
            } else {
                record_serialization::storage::qualify(field)?;
                if field.native_type() != SHORT_TEXT {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
            }
            rows::unique_identities(updates, budget, cancelled)?;
        }
    }
    let cell = group.cell(0).ok_or(EditError::Preservation)?;
    let valid = match field.native_type() {
        INT32 => {
            matches!(cell.expected, Value::Int32(_) | Value::Null)
                && matches!(cell.value, Value::Int32(_) | Value::Null)
        }
        BOOLEAN
            if matches!(
                family,
                Family::RetainedBoolean | Family::RetainedBooleanMaterialization
            ) =>
        {
            matches!(cell.expected, Value::Boolean(_)) && matches!(cell.value, Value::Boolean(_))
        }
        CURRENCY if matches!(family, Family::RetainedCurrency) => {
            matches!(cell.expected, Value::Currency(_) | Value::Null)
                && matches!(cell.value, Value::Currency(_) | Value::Null)
        }
        SHORT_TEXT => {
            matches!(cell.expected, Value::Text(_) | Value::Null)
                && matches!(cell.value, Value::Text(_) | Value::Null)
        }
        GUID | NUMERIC if matches!(family, Family::RetainedWideValues) => {
            record_serialization::current_rows::wide_value(field, cell.expected)
                && record_serialization::current_rows::wide_value(field, cell.value)
        }
        native if matches!(family, Family::RetainedFixedScalars) => {
            record_serialization::current_rows::fixed_scalar_value(native, cell.expected)
                && record_serialization::current_rows::fixed_scalar_value(native, cell.value)
        }
        _ => false,
    };
    if !valid {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if cell.row >= source.facts().records.len() {
        return Err(EditError::Conflict);
    }
    Ok(column)
}

#[cfg(test)]
mod tests;
