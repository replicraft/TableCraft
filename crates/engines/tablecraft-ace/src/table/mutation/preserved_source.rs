//! Projected source certification without manufacturing a full TableCheck.
mod dependencies;
mod footprint;
mod namemap;
pub(in crate::table::mutation) mod properties;
mod record_admission;
#[cfg(feature = "journal")]
mod retained_integral;
pub(in crate::table::mutation) mod validation;

use crate::{
    EditError, IndexInspection, InspectError, NativeImage, TableSnapshot, WriteBlocker,
    format::{
        allocation,
        data_type::{COMPLEX, INT32},
        layout::{CALCULATED_EXT_MASK, FIXED_FLAG, UPDATABLE_FLAG},
        reader::Reader,
    },
    header::check,
    inspect_relationships, inspect_table_indexes, inspect_table_properties, read_attachment_column,
    read_table_projection,
    table::{
        indexes::preserved,
        mutation::{gates, record_roots as roots, rows::RowAssignment},
        record_layout,
    },
};
use footprint::Footprint;
use record_admission::RecordAdmission;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

use crate::table::mutation::guards::fields::batch::SelectedPrograms;
#[cfg(feature = "journal")]
use crate::table::mutation::no_required_programs::NoRequiredTablePrograms;
#[cfg(feature = "journal")]
use crate::table::mutation::ordinary_content::{
    ClosedContent, definition::tail::RetainedDefinitionTail,
};

const CATALOG_HAS_COMPLEX: u32 = 0x0004_0000;

pub(in crate::table::mutation) struct Record {
    pub reference: u32,
    pub offset: usize,
    pub data: Vec<u8>,
}

pub(in crate::table::mutation) struct Source {
    pub table: TableSnapshot,
    pub indexes: IndexInspection,
    pub records: Vec<Record>,
    pub dependencies: Vec<usize>,
    #[cfg(feature = "journal")]
    definition_tail: Option<RetainedDefinitionTail>,
}

impl Source {
    #[cfg(feature = "journal")]
    pub(in crate::table::mutation) fn require_preserved_definition_tail(
        &self,
        image: &NativeImage,
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        if let Some(tail) = &self.definition_tail {
            tail.require_unchanged(
                image,
                after,
                self.table.schema().definition_page(),
                budget,
                cancelled,
            )?;
        }
        Ok(())
    }
}

struct Ordinal(usize);

impl RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

struct ComplexKeys {
    column: usize,
    keys: Vec<Option<i32>>,
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table::mutation) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_with_payload(image, table, PayloadAdmission::Inert, budget, cancelled)
}

/// Exact selected Extra admission; all other Source checks are shared unchanged.
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table::mutation) fn capture_before_change(
    image: &NativeImage,
    table: &str,
    selected: &SelectedPrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_with_payload(
        image,
        table,
        PayloadAdmission::Selected(selected),
        budget,
        cancelled,
    )
}

enum PayloadAdmission<'admission, 'image> {
    #[cfg(feature = "ace16-write-preview")]
    Inert,
    #[cfg(feature = "ace16-write-preview")]
    Selected(&'admission SelectedPrograms<'image>),
    #[cfg(feature = "journal")]
    NativeSelected(&'admission SelectedPrograms<'image>),
    #[cfg(feature = "journal")]
    NativeChecked(
        &'admission SelectedPrograms<'image>,
        &'admission crate::TableCheck,
    ),
    #[cfg(feature = "journal")]
    Closed(&'admission ClosedContent<'image>),
    #[cfg(feature = "journal")]
    NoRequired(&'admission NoRequiredTablePrograms<'image>),
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    CurrentSchemaNoRequired(&'admission NoRequiredTablePrograms<'image>),
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    CurrentRowsNoRequired(&'admission NoRequiredTablePrograms<'image>),
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    CurrentInsertNoRequired(&'admission NoRequiredTablePrograms<'image>),
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    RetainedInsertNoRequired(&'admission NoRequiredTablePrograms<'image>),
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    RetainedRowsNoRequired(&'admission NoRequiredTablePrograms<'image>),
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    CurrentDeleteNoRequired(&'admission NoRequiredTablePrograms<'image>),
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    HistoricalFixedRows(&'admission NoRequiredTablePrograms<'image>),
}

#[derive(Clone, Copy)]
enum ColumnFraming {
    Dense,
    #[cfg(feature = "journal")]
    Actions,
    #[cfg(feature = "journal")]
    SparseGenerated,
}

#[cfg(feature = "journal")]
#[derive(Clone, Copy)]
enum RetainedKeys {
    Integral,
    Scalar,
}

#[derive(Clone, Copy)]
struct ReadProfile {
    columns: ColumnFraming,
    #[cfg(feature = "journal")]
    keys: RetainedKeys,
}

impl ReadProfile {
    const fn dense() -> Self {
        Self {
            columns: ColumnFraming::Dense,
            #[cfg(feature = "journal")]
            keys: RetainedKeys::Integral,
        }
    }

    #[cfg(feature = "journal")]
    const fn retained(columns: ColumnFraming) -> Self {
        Self {
            columns,
            keys: RetainedKeys::Scalar,
        }
    }
}

fn capture_with_payload(
    image: &NativeImage,
    table: &str,
    payload: PayloadAdmission<'_, '_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed(
        image,
        table,
        payload,
        ReadProfile::dense(),
        budget,
        cancelled,
    )
}

fn capture_framed(
    image: &NativeImage,
    table: &str,
    payload: PayloadAdmission<'_, '_>,
    profile: ReadProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        payload,
        profile,
        RecordAdmission::Complete,
        budget,
        cancelled,
    )
}

fn capture_framed_with_records(
    image: &NativeImage,
    table: &str,
    payload: PayloadAdmission<'_, '_>,
    profile: ReadProfile,
    records_admission: RecordAdmission,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let framing = profile.columns;
    check(budget, cancelled)?;
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let schema = indexes.schema();
    if matches!(framing, ColumnFraming::Dense)
        && (schema.columns().len() != schema.layout.max_columns
            || schema
                .columns()
                .iter()
                .enumerate()
                .any(|(position, field)| field.number() != position))
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    for (position, field) in schema.columns().iter().enumerate() {
        for prior in &schema.columns()[..position] {
            check(budget, cancelled)?;
            name_work(field.name(), prior.name(), budget)?;
            if field.name().eq_ignore_ascii_case(prior.name()) {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    budget
        .charge_allocation(schema.columns().len() as u64, size_of::<&str>() as u64)
        .map_err(InspectError::from)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    let mut complex = Vec::new();
    budget
        .charge_allocation(
            schema.columns().len() as u64,
            size_of::<ComplexKeys>() as u64,
        )
        .map_err(InspectError::from)?;
    complex
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    let mut unavailable = false;

    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.scalar_type().is_some() {
            names.push(field.name());
        } else if field.native_type() == COMPLEX {
            unavailable = true;
            let attachments = read_attachment_column(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                table,
                field.name(),
                budget,
                cancelled,
            )?;
            let parents = attachments.links().parents();
            budget
                .charge_allocation(parents.len() as u64, size_of::<Option<i32>>() as u64)
                .map_err(InspectError::from)?;
            let mut keys = Vec::new();
            keys.try_reserve_exact(parents.len())
                .map_err(|_| InspectError::Allocation)?;
            for parent in parents {
                check(budget, cancelled)?;
                keys.push(parent.key());
            }
            complex.push(ComplexKeys {
                column: position,
                keys,
            });
        } else if field.extended_flags() & CALCULATED_EXT_MASK != 0 {
            unavailable = true;
        } else {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    if !unavailable && !payload.allows_scalar() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let projected = read_table_projection(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        &names,
        budget,
        cancelled,
    )?;
    drop(names);
    if projected.schema().object_id() != schema.object_id()
        || projected.schema().definition_page() != schema.definition_page()
        || projected.rows().len() != schema.declared_rows() as usize
    {
        return Err(EditError::Preservation);
    }
    gates::source_origin(
        image,
        schema,
        if complex.is_empty() {
            0
        } else {
            CATALOG_HAS_COMPLEX
        },
        budget,
        cancelled,
    )?;
    match &payload {
        #[cfg(feature = "ace16-write-preview")]
        PayloadAdmission::Inert => gates::inert_payloads(image, schema, budget, cancelled)?,
        #[cfg(feature = "ace16-write-preview")]
        PayloadAdmission::Selected(selected) => {
            selected.require_source(image, schema, budget, cancelled)?
        }
        #[cfg(feature = "journal")]
        PayloadAdmission::NativeSelected(selected)
        | PayloadAdmission::NativeChecked(selected, _) => {
            selected.require_source(image, schema, budget, cancelled)?
        }
        #[cfg(feature = "journal")]
        PayloadAdmission::NoRequired(certificate) => {
            certificate.require_source(image, schema, budget, cancelled)?
        }
        #[cfg(all(feature = "journal", feature = "ace16-create"))]
        PayloadAdmission::CurrentRowsNoRequired(certificate) => {
            certificate.require_current_rows_source(image, schema, budget, cancelled)?;
        }
        #[cfg(all(feature = "journal", feature = "ace16-create"))]
        PayloadAdmission::CurrentInsertNoRequired(certificate) => {
            certificate.require_current_insert_source(image, schema, budget, cancelled)?;
        }
        #[cfg(all(feature = "journal", feature = "ace16-create"))]
        PayloadAdmission::RetainedRowsNoRequired(certificate) => {
            certificate.require_retained_rows_source(image, schema, budget, cancelled)?;
        }
        #[cfg(all(feature = "journal", feature = "ace16-create"))]
        PayloadAdmission::RetainedInsertNoRequired(certificate) => {
            certificate.require_retained_insert_source(image, schema, budget, cancelled)?;
        }
        #[cfg(all(feature = "journal", feature = "ace16-create"))]
        PayloadAdmission::CurrentDeleteNoRequired(certificate) => {
            certificate.require_current_delete_source(image, schema, budget, cancelled)?;
        }
        #[cfg(all(feature = "journal", feature = "ace16-create"))]
        PayloadAdmission::CurrentSchemaNoRequired(certificate) => {
            certificate.require_current_schema_source(image, schema, budget, cancelled)?;
        }
        #[cfg(all(feature = "journal", feature = "ace16-create"))]
        PayloadAdmission::HistoricalFixedRows(certificate) => {
            certificate.require_historical_fixed_rows_source(image, schema, budget, cancelled)?
        }
        #[cfg(feature = "journal")]
        PayloadAdmission::Closed(certificate) => {
            certificate.require_source(image, schema, budget, cancelled)?
        }
    }
    #[cfg(feature = "journal")]
    let definition_tail = match &payload {
        PayloadAdmission::NoRequired(certificate) => certificate.share_definition_tail(),
        #[cfg(feature = "ace16-create")]
        PayloadAdmission::CurrentSchemaNoRequired(certificate)
        | PayloadAdmission::CurrentRowsNoRequired(certificate)
        | PayloadAdmission::CurrentInsertNoRequired(certificate)
        | PayloadAdmission::RetainedInsertNoRequired(certificate)
        | PayloadAdmission::RetainedRowsNoRequired(certificate)
        | PayloadAdmission::CurrentDeleteNoRequired(certificate)
        | PayloadAdmission::HistoricalFixedRows(certificate) => certificate.share_definition_tail(),
        _ => None,
    };
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    properties::qualify(schema, &properties, budget, cancelled)?;
    crate::constraints::check_scalar_projection(&projected, &properties, budget, cancelled)?;
    let dependencies = dependencies::capture(image, schema, budget, cancelled)?;
    for relationship in inspect_relationships(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )? {
        check(budget, cancelled)?;
        let declaration = relationship.declaration();
        if declaration
            .parent_table()
            .as_str()
            .eq_ignore_ascii_case(table)
            || declaration
                .child_table()
                .as_str()
                .eq_ignore_ascii_case(table)
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
    }

    budget
        .charge_allocation(projected.rows().len() as u64, size_of::<Ordinal>() as u64)
        .map_err(InspectError::from)?;
    let mut ordinals = Vec::new();
    ordinals
        .try_reserve_exact(projected.rows().len())
        .map_err(|_| InspectError::Allocation)?;
    for ordinal in 0..projected.rows().len() {
        check(budget, cancelled)?;
        ordinals.push(Ordinal(ordinal));
    }
    let roots = roots::capture(image, schema, &ordinals, budget, cancelled)?;
    let global = {
        let mut bytes = image.reader();
        let mut reader = Reader {
            source: &mut bytes,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        allocation::RepresentedMap::read(
            &mut reader,
            allocation::GLOBAL_FREE_REFERENCE,
            allocation::MapRole::GlobalFree,
        )?
    };
    allocated(image, &global, &roots.owned, budget, cancelled)?;
    let metadata = Footprint::capture(
        image,
        schema,
        indexes.indexes(),
        &payload,
        budget,
        cancelled,
    )?;
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    let long_values = match &payload {
        PayloadAdmission::RetainedRowsNoRequired(_) => {
            crate::table::mutation::lval_ownership::Inventory::capture_retained(
                image, budget, cancelled,
            )?
        }
        _ => crate::table::mutation::lval_ownership::Inventory::capture(image, budget, cancelled)?,
    };
    #[cfg(not(all(feature = "journal", feature = "ace16-create")))]
    let long_values =
        crate::table::mutation::lval_ownership::Inventory::capture(image, budget, cancelled)?;

    budget
        .charge_allocation(roots.selected.len() as u64, size_of::<Record>() as u64)
        .map_err(InspectError::from)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(roots.selected.len())
        .map_err(|_| InspectError::Allocation)?;
    for root in roots.selected {
        check(budget, cancelled)?;
        metadata.require_record(
            image,
            schema,
            root.final_record.offset,
            root.final_record.data.len(),
            budget,
            cancelled,
        )?;
        let layout = record_layout::parse(&root.final_record.data, schema, budget)?;
        records_admission.validate(
            &layout,
            schema,
            projected
                .rows()
                .get(records.len())
                .ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        records.push(Record {
            reference: root.reference,
            offset: root.final_record.offset,
            data: root.final_record.data,
        });
    }
    if records.len() != projected.rows().len() {
        return Err(EditError::Preservation);
    }

    #[cfg(feature = "journal")]
    if let PayloadAdmission::NativeChecked(_, checked) = &payload {
        if checked.schema().object_id() != schema.object_id()
            || checked.schema().definition_page() != schema.definition_page()
            || checked.rows() != projected.rows().len()
            || projected.columns().len() != schema.columns().len()
            || checked
                .indexes()
                .iter()
                .any(|index| index.status() != crate::IndexCheckStatus::Checked)
        {
            return Err(EditError::Preservation);
        }
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.scalar_type().is_none() || field.extended_flags() & CALCULATED_EXT_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        return Ok(Source {
            table: projected,
            indexes,
            records,
            dependencies,
            #[cfg(feature = "journal")]
            definition_tail,
        });
    }

    #[cfg(feature = "journal")]
    let retain_native_keys = match &payload {
        PayloadAdmission::Closed(_) | PayloadAdmission::NoRequired(_) => true,
        #[cfg(feature = "ace16-create")]
        PayloadAdmission::CurrentSchemaNoRequired(_)
        | PayloadAdmission::CurrentRowsNoRequired(_)
        | PayloadAdmission::CurrentInsertNoRequired(_)
        | PayloadAdmission::RetainedInsertNoRequired(_)
        | PayloadAdmission::RetainedRowsNoRequired(_)
        | PayloadAdmission::CurrentDeleteNoRequired(_)
        | PayloadAdmission::HistoricalFixedRows(_) => true,
        _ => false,
    };

    #[cfg(feature = "journal")]
    let retained_integral = retained_integral::qualify(
        image,
        &indexes,
        &projected,
        retain_native_keys.then_some(profile),
        &long_values,
        budget,
        cancelled,
    )?;

    let mut physical: Vec<usize> = Vec::new();
    budget
        .charge_allocation(indexes.indexes().len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    physical
        .try_reserve_exact(indexes.indexes().len())
        .map_err(|_| InspectError::Allocation)?;
    for (position, index) in indexes.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        #[cfg(feature = "journal")]
        let retained_tuple =
            retained_tuple_scope(profile, retained_integral, index.columns().len());
        #[cfg(not(feature = "journal"))]
        let retained_tuple = false;
        let scalar_certification = if retained_tuple {
            // qualify() already proved every actual tuple alias, eligible
            // membership, complete owned closure and catalog/LVAL roles.
            None
        } else {
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            };
            let column = match framing {
                ColumnFraming::Dense => schema.columns().get(key.number()),
                #[cfg(feature = "journal")]
                ColumnFraming::Actions | ColumnFraming::SparseGenerated => {
                    budget
                        .charge(Resource::Steps, schema.columns().len() as u64)
                        .map_err(InspectError::from)?;
                    schema
                        .columns()
                        .iter()
                        .find(|field| field.number() == key.number())
                }
            }
            .ok_or(InspectError::Malformed)?;
            Some((key, column))
        };
        if index.native_type() > 1 || index.constraints().is_none() {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        if let Some((_, column)) = scalar_certification
            && index
                .constraints()
                .is_some_and(|flags| flags.ignore_nulls())
        {
            // Read certification covers only a manual native Int32 secondary.
            // Every logical alias is checked before physical-owner deduplication.
            if column.native_type() != INT32
                || column.scalar_type() != Some(tablecraft_schema::ScalarType::Int32)
                || !column.fixed()
                || column.storage_length() != size_of::<i32>()
                || column.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || column.extended_flags() != 0
                || column.complex_type_id().is_some()
            {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            for alias in indexes.indexes() {
                check(budget, cancelled)?;
                if alias.data_number() == index.data_number()
                    && alias.constraints().is_none_or(|flags| flags.primary_key())
                {
                    return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
                }
            }
        }
        budget
            .charge(Resource::Steps, physical.len() as u64)
            .map_err(InspectError::from)?;
        if let Some(prior) = physical
            .iter()
            .map(|position| &indexes.indexes()[*position])
            .find(|prior| prior.data_number() == index.data_number())
        {
            if prior.root_page() != index.root_page()
                || prior.usage_map() != index.usage_map()
                || prior.flags() != index.flags()
                || prior.columns() != index.columns()
            {
                return Err(EditError::Preservation);
            }
            continue;
        }
        physical.push(position);
        #[cfg(feature = "journal")]
        let retained_pages = if retained_integral {
            Some(retained_integral::pages(image, index, budget, cancelled)?)
        } else {
            None
        };
        #[cfg(not(feature = "journal"))]
        let retained_pages: Option<Vec<u32>> = None;
        let pages = if let Some(pages) = retained_pages {
            pages
        } else {
            // A retained tuple must never enter the scalar fallback certifier.
            let (key, column) = scalar_certification
                .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
            budget
                .charge_allocation(records.len() as u64, size_of::<(u32, Option<i32>)>() as u64)
                .map_err(InspectError::from)?;
            let mut expected = Vec::new();
            expected
                .try_reserve_exact(records.len())
                .map_err(|_| InspectError::Allocation)?;
            if column.native_type() == COMPLEX {
                let keys = complex
                    .iter()
                    .find(|keys| keys.column == key.number())
                    .ok_or(InspectError::Malformed)?;
                if keys.keys.len() != records.len() {
                    return Err(InspectError::Malformed.into());
                }
                for (root, key) in records.iter().zip(&keys.keys) {
                    check(budget, cancelled)?;
                    expected.push((root.reference, *key));
                }
            } else if column.native_type() == INT32
                && column.scalar_type() == Some(tablecraft_schema::ScalarType::Int32)
            {
                for field in projected.columns() {
                    name_work(field.name(), column.name(), budget)?;
                }
                let dense = projected
                    .columns()
                    .iter()
                    .position(|field| field.name() == column.name())
                    .ok_or(InspectError::Malformed)?;
                for (root, row) in records.iter().zip(projected.rows()) {
                    check(budget, cancelled)?;
                    let key = match row.get(dense).ok_or(InspectError::Malformed)? {
                        Value::Null => None,
                        Value::Int32(value) => Some(*value),
                        _ => return Err(EditError::Preservation),
                    };
                    expected.push((root.reference, key));
                }
            } else {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            preserved::certify(
                image,
                schema.definition_page(),
                index,
                &expected,
                budget,
                cancelled,
            )?
        };
        for page in pages {
            allocated(image, &global, &[page], budget, cancelled)?;
            // No index page may also be an ordinary DATA page. Metadata protects
            // every INDEX body; the explicit owned census establishes disjointness.
            budget
                .charge(Resource::Steps, roots.owned.len() as u64)
                .map_err(InspectError::from)?;
            if roots.owned.contains(&page) {
                return Err(EditError::Preservation);
            }
        }
    }
    if physical.len() != schema.layout.physical_indexes {
        return Err(EditError::Preservation);
    }
    for number in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, physical.len() as u64)
            .map_err(InspectError::from)?;
        if !physical
            .iter()
            .any(|position| indexes.indexes()[*position].data_number() as usize == number)
        {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(Source {
        table: projected,
        indexes,
        records,
        dependencies,
        #[cfg(feature = "journal")]
        definition_tail,
    })
}

#[cfg(feature = "journal")]
fn retained_tuple_scope(profile: ReadProfile, retained: bool, keys: usize) -> bool {
    retained
        && matches!(profile.columns, ColumnFraming::Dense)
        && matches!(profile.keys, RetainedKeys::Scalar)
        && (2..=crate::format::layout::INDEX_KEY_COLUMNS).contains(&keys)
}

fn allocated(
    image: &NativeImage,
    global: &allocation::RepresentedMap,
    pages: &[u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut bytes = image.reader();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    global.require_allocated(&mut reader, 0)?;
    global.require_allocated(&mut reader, 1)?;
    for &page in pages {
        reader.tick()?;
        global.require_allocated(&mut reader, page)?;
    }
    Ok(())
}

impl PayloadAdmission<'_, '_> {
    fn allows_scalar(&self) -> bool {
        match self {
            #[cfg(feature = "journal")]
            Self::NativeSelected(_)
            | Self::NativeChecked(_, _)
            | Self::Closed(_)
            | Self::NoRequired(_) => true,
            #[cfg(all(feature = "journal", feature = "ace16-create"))]
            Self::CurrentSchemaNoRequired(_)
            | Self::CurrentRowsNoRequired(_)
            | Self::CurrentInsertNoRequired(_)
            | Self::RetainedInsertNoRequired(_)
            | Self::RetainedRowsNoRequired(_)
            | Self::CurrentDeleteNoRequired(_)
            | Self::HistoricalFixedRows(_) => true,
            #[cfg(feature = "ace16-write-preview")]
            Self::Inert | Self::Selected(_) => false,
        }
    }
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_native_before_change(
    image: &NativeImage,
    table: &str,
    selected: &SelectedPrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_with_payload(
        image,
        table,
        PayloadAdmission::NativeSelected(selected),
        budget,
        cancelled,
    )
}

fn name_work(left: &str, right: &str, budget: &mut ResourceBudget) -> Result<(), EditError> {
    budget
        .charge(Resource::Steps, left.len().max(right.len()) as u64)
        .map_err(InspectError::from)?;
    Ok(())
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_checked_before_change(
    image: &NativeImage,
    table: &str,
    selected: &SelectedPrograms<'_>,
    checked: &crate::TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_with_payload(
        image,
        table,
        PayloadAdmission::NativeChecked(selected, checked),
        budget,
        cancelled,
    )
}

/// Actual ordinary-content proof replaces neither source validation nor field
/// admission. It must name this exact image and an actual user-table role.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_closed_content(
    image: &NativeImage,
    table: &str,
    certificate: &ClosedContent<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_with_payload(
        image,
        table,
        PayloadAdmission::Closed(certificate),
        budget,
        cancelled,
    )
}

/// Bounded native target certification retains the complete normal Source
/// protection, properties, dependencies, relationships and index checks.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_no_required_programs(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_with_payload(
        image,
        table,
        PayloadAdmission::NoRequired(certificate),
        budget,
        cancelled,
    )
}

/// Full action Source; the same native absence certificate and all preservation
/// checks remain required. No UPDATE/event caller selects this framing.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_no_required_actions(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed(
        image,
        table,
        PayloadAdmission::NoRequired(certificate),
        ReadProfile::retained(ColumnFraming::Actions),
        budget,
        cancelled,
    )
}

/// Current-schema observation accepts retained native ordinals only with its
/// own source-bound absence certificate, never a serialized permission.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) fn capture_current_schema(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::CurrentSchemaNoRequired(certificate),
        ReadProfile::retained(ColumnFraming::Actions),
        RecordAdmission::HistoricalSchemaReadonly,
        budget,
        cancelled,
    )
}

/// Current row operations retain their own positive program/source purpose.
/// Schema-only observations cannot be passed here as row-mutation authority.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) fn capture_current_rows(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::CurrentRowsNoRequired(certificate),
        ReadProfile::retained(ColumnFraming::Actions),
        RecordAdmission::CurrentRows,
        budget,
        cancelled,
    )
}

/// Preserve original historical rows under a distinct current-INSERT purpose.
/// The consuming writer must independently prove every newly encoded row,
/// generated value, physical placement and index/counter change.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) fn capture_current_insert(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::CurrentInsertNoRequired(certificate),
        ReadProfile::retained(ColumnFraming::Actions),
        RecordAdmission::HistoricalInsert,
        budget,
        cancelled,
    )
}

/// Retained replacement retains its own row-purpose witness. Shared historical
/// record reading does not select an insertion or schema mutation capability.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) fn capture_retained_rows(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::RetainedRowsNoRequired(certificate),
        ReadProfile {
            columns: ColumnFraming::Actions,
            keys: RetainedKeys::Integral,
        },
        RecordAdmission::CurrentRows,
        budget,
        cancelled,
    )
}

/// Original generated records retain their own insertion grammar while the
/// bound certificate protects retired catalog/table storage outside the target.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) fn capture_retained_insert(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::RetainedInsertNoRequired(certificate),
        ReadProfile {
            columns: ColumnFraming::Actions,
            keys: RetainedKeys::Integral,
        },
        RecordAdmission::HistoricalInsert,
        budget,
        cancelled,
    )
}

/// Current DELETE retains historical bytes until independently proved root
/// tombstoning and index removal. A readonly certificate cannot enter this path.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) fn capture_current_delete(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::CurrentDeleteNoRequired(certificate),
        ReadProfile::retained(ColumnFraming::Actions),
        RecordAdmission::CurrentDelete,
        budget,
        cancelled,
    )
}

/// Dedicated historical fixed-row observation, never ordinary UPDATE admission.
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) fn capture_historical_fixed_rows(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::HistoricalFixedRows(certificate),
        ReadProfile::retained(ColumnFraming::Actions),
        RecordAdmission::HistoricalFixedRows,
        budget,
        cancelled,
    )
}

/// INSERT-only sparse identities retain the complete actual absence certificate.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_no_required_generated_insert(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    certificate.require_generated_insert()?;
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::NoRequired(certificate),
        ReadProfile {
            columns: ColumnFraming::SparseGenerated,
            keys: RetainedKeys::Integral,
        },
        RecordAdmission::HistoricalInsert,
        budget,
        cancelled,
    )
}

/// INSERT-only original records may predate later native field declarations.
/// Existing values remain immutable; ordinary UPDATE keeps complete coverage.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_no_required_insert(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let profile = if certificate.retained_update_keys()? {
        ReadProfile::retained(ColumnFraming::Dense)
    } else {
        ReadProfile::dense()
    };
    capture_framed_with_records(
        image,
        table,
        PayloadAdmission::NoRequired(certificate),
        profile,
        RecordAdmission::HistoricalInsert,
        budget,
        cancelled,
    )
}

/// Dense native UPDATE Source retains readonly keys without issuing target facts.
#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn capture_no_required_updates(
    image: &NativeImage,
    table: &str,
    certificate: &NoRequiredTablePrograms<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    check(budget, cancelled)?;
    if !certificate.retained_update_keys()? {
        return capture_no_required_programs(image, table, certificate, budget, cancelled);
    }
    capture_framed(
        image,
        table,
        PayloadAdmission::NoRequired(certificate),
        ReadProfile::retained(ColumnFraming::Dense),
        budget,
        cancelled,
    )
}

#[cfg(all(test, feature = "journal"))]
mod retained_tuple_scope_tests {
    use super::*;

    #[test]
    fn retained_tuple_branch_requires_the_actual_closed_dense_scalar_profile() {
        let scalar = ReadProfile {
            columns: ColumnFraming::Dense,
            keys: RetainedKeys::Scalar,
        };
        assert!(retained_tuple_scope(scalar, true, 2));
        assert!(retained_tuple_scope(scalar, true, 10));
        for count in [0, 1, 11] {
            assert!(!retained_tuple_scope(scalar, true, count));
        }
        assert!(!retained_tuple_scope(scalar, false, 2));
        assert!(!retained_tuple_scope(
            ReadProfile {
                columns: ColumnFraming::Dense,
                keys: RetainedKeys::Integral
            },
            true,
            2,
        ));
        assert!(!retained_tuple_scope(
            ReadProfile {
                columns: ColumnFraming::Actions,
                keys: RetainedKeys::Scalar
            },
            true,
            2,
        ));
    }
}
