//! Positive schema preservation scopes do not authorize any INDEX or ID rewrite.
use super::CurrentFieldAdditionError as Error;
use crate::{
    IndexInspection, NativeImage, PropertyGroupKind, TableSchema,
    format::{
        data_type::INT32,
        layout::{AUTO_NUMBER_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    inspect_table_indexes,
    table::mutation::{
        current_schema::{self, Family},
        normal_fitted::binding,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const GENERATED_I32_FLAGS: u8 = FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG;
const LAST_AT: usize = 20;
const GENERATED_HEADER_END: usize = 40;

/// A source language selected from actual captured declarations, never a caller flag.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceProfile {
    Keyless,
    Indexed,
}

/// The full source proof and its positive source class remain one immutable owner.
pub(super) struct Source {
    proof: current_schema::Source,
    profile: SourceProfile,
}

impl Source {
    pub(super) fn proof(&self) -> &current_schema::Source {
        &self.proof
    }

    pub(super) const fn profile(&self) -> SourceProfile {
        self.profile
    }
}

pub(super) fn capture(
    image: NativeImage,
    table: &str,
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, Error> {
    let proof = Family::Retained.capture(image, table, budget, cancelled)?;
    let profile = classify(&proof.table().schema)?;
    let source = Source { proof, profile };
    qualify(&source, new_name, budget, cancelled)?;
    Ok(source)
}

pub(super) fn qualify(
    source: &Source,
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let table = source.proof.table();
    if table.schema.columns().len() >= crate::limits::MAX_USER_COLUMNS {
        return Err(Error::Capacity);
    }
    qualify_shape(
        source.proof.image(),
        &table.schema,
        source.profile(),
        budget,
        cancelled,
    )?;
    for field in table.schema.columns() {
        check(budget, cancelled)?;
        name_work(field.name(), new_name, budget)?;
        if field.name().eq_ignore_ascii_case(new_name) {
            return Err(Error::DuplicateName);
        }
    }
    for group in table.properties.groups() {
        check(budget, cancelled)?;
        if group.kind() == PropertyGroupKind::Column {
            name_work(group.name(), new_name, budget)?;
            if !group.name().is_ascii() || group.name().eq_ignore_ascii_case(new_name) {
                return Err(Error::Source);
            }
        }
    }
    Ok(())
}

fn classify(schema: &TableSchema) -> Result<SourceProfile, Error> {
    let layout = schema.physical_layout();
    match (layout.physical_indexes, layout.logical_indexes) {
        (0, 0) => Ok(SourceProfile::Keyless),
        (physical, logical) if physical > 0 && logical > 0 => Ok(SourceProfile::Indexed),
        _ => Err(Error::Source),
    }
}

fn qualify_shape(
    image: &NativeImage,
    schema: &TableSchema,
    profile: SourceProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexInspection, Error> {
    if classify(schema)? != profile {
        return Err(Error::Source);
    }
    let mut counter = None;
    for field in schema.columns() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, field.name().len() as u64)?;
        if !field.name().is_ascii() {
            return Err(Error::Source);
        }
        let ordinary = UPDATABLE_FLAG | if field.fixed() { FIXED_FLAG } else { 0 };
        if field.flags() == ordinary {
            continue;
        }
        if profile != SourceProfile::Indexed
            || field.flags() != GENERATED_I32_FLAGS
            || field.native_type() != INT32
            || field.scalar_type() != Some(ScalarType::Int32)
            || !field.fixed()
            || field.storage_length() != size_of::<i32>()
            || field.extended_flags() != 0
            || field.complex_type_id().is_some()
            || counter.replace(field.number()).is_some()
        {
            return Err(Error::Source);
        }
    }
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if indexes.schema().object_id() != schema.object_id()
        || indexes.schema().definition_page() != schema.definition_page()
        || indexes.schema().columns().len() != schema.columns().len()
    {
        return Err(Error::Preservation);
    }
    let mut primary_counter = false;
    for index in indexes.indexes() {
        check(budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(Error::Source);
        };
        budget.charge(Resource::Steps, schema.columns().len() as u64)?;
        let field = schema
            .columns()
            .iter()
            .find(|field| field.number() == key.number())
            .ok_or(Error::Source)?;
        if field.native_type() != INT32
            || field.scalar_type() != Some(ScalarType::Int32)
            || !field.fixed()
            || field.storage_length() != size_of::<i32>()
            || field.extended_flags() != 0
            || field.complex_type_id().is_some()
            || index.foreign_reference().is_some()
        {
            return Err(Error::Source);
        }
        let constraints = index.constraints().ok_or(Error::Source)?;
        primary_counter |= constraints.primary_key() && counter == Some(field.number());
    }
    if counter.is_some() && !primary_counter {
        return Err(Error::Source);
    }
    Ok(indexes)
}

/// Full source admission is already repeated by the caller's fresh Family observe.
/// These independent invariants prevent metadata shifts from reinterpreting old keys.
pub(super) fn prove_preserved(
    source: &Source,
    after: &NativeImage,
    observed: &current_schema::Table,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let before = source.proof.image();
    let old = &source.proof.table().schema;
    let new = &observed.schema;
    if before.as_bytes().len() != after.as_bytes().len()
        || old.object_id() != new.object_id()
        || old.definition_page() != new.definition_page()
        || old.declared_rows() != new.declared_rows()
        || new.columns().len() != old.columns().len() + 1
    {
        return Err(Error::Preservation);
    }
    for (left, right) in old.columns().iter().zip(new.columns()) {
        check(budget, cancelled)?;
        name_work(left.name(), right.name(), budget)?;
        if left.name() != right.name()
            || !left.physical_field().same_storage(right.physical_field())
        {
            return Err(Error::Preservation);
        }
    }
    let old_indexes = qualify_shape(before, old, source.profile(), budget, cancelled)?;
    let new_indexes = qualify_shape(after, new, source.profile(), budget, cancelled)?;
    if old_indexes.indexes().len() != new_indexes.indexes().len() {
        return Err(Error::Preservation);
    }
    for (left, right) in old_indexes.indexes().iter().zip(new_indexes.indexes()) {
        check(budget, cancelled)?;
        name_work(left.name(), right.name(), budget)?;
        if left.name() != right.name()
            || left.number() != right.number()
            || left.data_number() != right.data_number()
            || left.root_page() != right.root_page()
            || left.usage_map() != right.usage_map()
            || left.native_type() != right.native_type()
            || left.flags() != right.flags()
            || left.columns() != right.columns()
        {
            return Err(Error::Preservation);
        }
    }
    let base = old.definition_page() as usize * PAGE_BYTES;
    let end = base.checked_add(PAGE_BYTES).ok_or(Error::Preservation)?;
    let left = before.as_bytes();
    let right = after.as_bytes();
    budget.charge(Resource::Steps, (GENERATED_HEADER_END - LAST_AT) as u64)?;
    if left.get(base + LAST_AT..base + GENERATED_HEADER_END)
        != right.get(base + LAST_AT..base + GENERATED_HEADER_END)
        || !binding::equal_bytes(&left[..base], &right[..base], budget, cancelled)?
        || !binding::equal_bytes(&left[end..], &right[end..], budget, cancelled)?
    {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

fn name_work(left: &str, right: &str, budget: &mut ResourceBudget) -> Result<(), Error> {
    budget.charge(Resource::Steps, left.len().max(right.len()) as u64)?;
    Ok(())
}

#[cfg(test)]
mod tests;
