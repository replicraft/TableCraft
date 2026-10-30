//! Projected source certification without manufacturing a full TableCheck.
use super::{dependencies, properties};
use crate::{
    EditError, IndexInspection, InspectError, NativeImage, TableSnapshot, WriteBlocker,
    format::{
        allocation,
        data_type::{COMPLEX, INT32},
        layout::CALCULATED_EXT_MASK,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    inspect_relationships, inspect_table_indexes, inspect_table_properties, read_attachment_column,
    read_table_projection,
    table::{
        indexes::preserved,
        mutation::{
            gates, page_update::roots, rows::RowAssignment, variable_split::metadata::Metadata,
        },
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const CATALOG_HAS_COMPLEX: u32 = 0x0004_0000;

pub(super) struct Record {
    pub reference: u32,
    pub offset: usize,
    pub data: Vec<u8>,
}

pub(super) struct Source {
    pub table: TableSnapshot,
    pub indexes: IndexInspection,
    pub records: Vec<Record>,
    pub dependencies: Vec<usize>,
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

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    check(budget, cancelled)?;
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let schema = indexes.schema();
    if schema.columns().len() != schema.layout.max_columns
        || schema
            .columns()
            .iter()
            .enumerate()
            .any(|(position, field)| field.number() != position)
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    for (position, field) in schema.columns().iter().enumerate() {
        for prior in &schema.columns()[..position] {
            check(budget, cancelled)?;
            super::name_work(field.name(), prior.name(), budget)?;
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
    if !unavailable {
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
    gates::inert_payloads(image, schema, budget, cancelled)?;
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
    let metadata = Metadata::capture_for_preserved_boolean(
        image,
        schema,
        indexes.indexes(),
        budget,
        cancelled,
    )?;
    metadata.require_source(image, schema.definition_page())?;
    let long_values =
        super::super::super::long_value::ownership::Inventory::capture(image, budget, cancelled)?;
    long_values.require_no_long_value_data_overlap(&roots.owned, budget, cancelled)?;
    budget
        .charge_allocation(roots.selected.len() as u64, size_of::<Record>() as u64)
        .map_err(InspectError::from)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(roots.selected.len())
        .map_err(|_| InspectError::Allocation)?;
    for root in roots.selected {
        check(budget, cancelled)?;
        metadata.require_data_page(
            (root.final_record.offset / PAGE_BYTES) as u32,
            budget,
            cancelled,
        )?;
        let layout = record_layout::parse(&root.final_record.data, schema, budget)?;
        if layout.count != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        records.push(Record {
            reference: root.reference,
            offset: root.final_record.offset,
            data: root.final_record.data,
        });
    }
    if records.len() != projected.rows().len() {
        return Err(EditError::Preservation);
    }

    let mut physical: Vec<usize> = Vec::new();
    budget
        .charge_allocation(indexes.indexes().len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    physical
        .try_reserve_exact(indexes.indexes().len())
        .map_err(|_| InspectError::Allocation)?;
    for (position, index) in indexes.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        };
        let column = schema
            .columns()
            .get(key.number())
            .ok_or(InspectError::Malformed)?;
        if index.native_type() > 1 || index.constraints().is_none() {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
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
                super::name_work(field.name(), column.name(), budget)?;
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
        let pages = preserved::certify(
            image,
            schema.definition_page(),
            index,
            &expected,
            budget,
            cancelled,
        )?;
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
    })
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
