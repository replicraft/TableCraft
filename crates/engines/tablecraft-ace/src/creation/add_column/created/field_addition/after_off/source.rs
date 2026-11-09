//! Capture declarations only after the owned common settled-source proof.
use super::{FieldAdditionAfterOffError as Error, FieldAdditionError};
use crate::{
    DatabaseProperties, FirstTableDefinition, NativeImage, TableCheck,
    creation::{FieldHistoryOffSource, properties::Order},
    format::reader::Reader,
    header::check,
    inspect_database_properties, inspect_table_properties, inspect_table_schema,
    table::mutation::catalog_property::{column, descriptor, ledger},
};
use std::ops::Range;
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Source {
    pub proof: FieldHistoryOffSource,
    pub checked: TableCheck,
    pub definition: FirstTableDefinition,
    pub order: Order,
    pub descriptor: Range<usize>,
    pub user_reference: u32,
    pub database: DatabaseProperties,
    pub database_reference: u32,
}

impl Source {
    pub(super) fn capture(
        image: NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let proof = FieldHistoryOffSource::capture(image, budget, cancelled)?;
        let image = proof.image();
        let (checked, definition) =
            super::super::super::source::inspect_field_addition(image, table, budget, cancelled)
                .map_err(FieldAdditionError::from)?;
        // Duplicate checks below use the existing ASCII-fold comparison owner.
        // Qualify every name in that comparison namespace before deriving a target.
        check(budget, cancelled)?;
        if !checked.schema().name().is_ascii() {
            return Err(Error::Namespace);
        }
        for column in checked.schema().columns() {
            check(budget, cancelled)?;
            budget.charge(tablecraft_core::limits::Resource::Steps, 1)?;
            if !column.name().is_ascii() {
                return Err(Error::Namespace);
            }
        }

        let scope = image.catalog().catalog().scope();
        let properties =
            inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
        let order = Order::capture(&properties, budget, cancelled)?;
        let database = inspect_database_properties(&mut image.reader(), scope, budget, cancelled)?
            .ok_or(Error::Preservation)?;
        let catalog =
            inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
        let selected = column(&catalog)?;
        let origin = image
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == checked.schema().object_id())
            .ok_or(Error::Preservation)?;
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let observed = descriptor::capture(&mut reader, &catalog, selected, origin)?;
        let descriptor = observed.absolute_range()?.ok_or(Error::Preservation)?;
        let ledger = ledger::capture(image.as_bytes(), image.catalog(), budget, cancelled)?;
        let (user_reference, user_length) = ledger
            .single_reference(checked.schema().object_id(), budget, cancelled)?
            .ok_or(Error::Preservation)?;
        let (database_reference, database_length) = ledger
            .single_reference(database.object_id(), budget, cancelled)?
            .ok_or(Error::Preservation)?;
        if user_length != properties.raw_bytes().len()
            || database_length != database.raw_bytes().len()
            || user_reference == database_reference
        {
            return Err(Error::Preservation);
        }
        check(budget, cancelled)?;
        Ok(Self {
            proof,
            checked,
            definition,
            order,
            descriptor,
            user_reference,
            database,
            database_reference,
        })
    }
}
