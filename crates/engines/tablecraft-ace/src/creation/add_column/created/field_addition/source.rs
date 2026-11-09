//! Exact mixed declaration/property history, reconstructed independently of raw groups.
use super::{FieldAdditionError as Error, FieldAdditionTarget, build};
use crate::{
    FirstTableDefinition, NativeImage, TableCheck, creation::properties::Order, header::check,
    inspect_table_properties, table::mutation::catalog_property,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Source {
    pub checked: TableCheck,
    pub definition: FirstTableDefinition,
    pub order: Order,
    pub external: bool,
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, Error> {
    check(budget, cancelled)?;
    let (checked, definition) =
        super::super::source::inspect_field_addition(image, table, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let properties =
        inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
    if properties.object_id() != checked.schema().object_id()
        || properties.name() != checked.schema().name()
    {
        return Err(Error::Source);
    }
    let order = Order::capture(&properties, budget, cancelled)?;
    let external =
        catalog_property::properties_external(image, checked.schema(), budget, cancelled)?;
    let count = checked.schema().columns().len();
    let history = if external {
        super::super::super::property_history::count(image, count, budget, cancelled)?
    } else {
        0
    };
    if history >= count || (external && history == 0) {
        return Err(Error::Source);
    }
    let initial = count - history;
    let columns = definition.borrowed_columns(budget, cancelled)?;
    let generated = columns.first().ok_or(Error::Source)?;
    if generated.required().is_some()
        || generated.allow_zero_length().is_some()
        || generated.default_literal().is_some()
        || generated.description().is_some()
        || generated.unicode_compression()
    {
        return Err(Error::Source);
    }
    // The initial prefix retains the constructive owner's checked inert
    // declarations. Only the historical appended suffix must belong to this
    // operation's closed target tuples. Canonical seed and complete history
    // equality below independently prove every prefix policy and physical byte.
    let mut reconstructed_order = build::initial_order(&columns[..initial], budget, cancelled)?;
    let seed = build::canonical(
        checked.schema().name(),
        &columns[..initial],
        reconstructed_order,
        budget,
        cancelled,
    )?;
    let mut reconstructed = NativeImage::capture(&mut Cursor::new(seed), scope, budget, cancelled)?;
    for position in initial..count {
        check(budget, cancelled)?;
        let target = FieldAdditionTarget::from_declaration(&columns[position])?;
        let (stage, _) =
            super::super::source::inspect_field_addition(&reconstructed, table, budget, cancelled)?;
        reconstructed_order = build::extend_order(
            reconstructed_order,
            &columns[..=position],
            target,
            budget,
            cancelled,
        )?;
        let after = build::append(
            &reconstructed,
            build::Addition {
                checked: &stage,
                columns: &columns[..=position],
                order: reconstructed_order,
                external: position != initial,
                target,
            },
            budget,
            cancelled,
        )?;
        reconstructed = NativeImage::capture(&mut Cursor::new(after), scope, budget, cancelled)?;
    }
    if reconstructed_order != order {
        return Err(Error::Source);
    }
    if !super::super::super::common::equal_image(
        image.as_bytes(),
        reconstructed.as_bytes(),
        budget,
        cancelled,
    )? {
        return Err(Error::Source);
    }
    drop(columns);
    Ok(Source {
        checked,
        definition,
        order,
        external,
    })
}
