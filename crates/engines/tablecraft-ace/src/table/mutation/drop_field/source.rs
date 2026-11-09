use super::super::saved_field::Observed;
use super::{FieldDropError as Error, NativeFieldDropIdentity};
use crate::{
    NativeImage, check_table,
    creation::{FieldHistoryOffSource, existing_fields, properties},
    format::reader::Reader,
    header::check,
    inspect_table_schema,
    properties::framing,
    table::mutation::catalog_property::{column, descriptor},
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Source {
    pub proof: FieldHistoryOffSource,
    pub evidence: Evidence,
}

/// Metadata alone is not mutation authority; Source retains the consumed proof.
pub(super) struct Evidence {
    pub observed: Observed,
    pub descriptor: Range<usize>,
    pub body: Vec<u8>,
}

impl Source {
    pub fn capture(
        image: NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        request(table, field, budget, cancelled)?;
        let proof = FieldHistoryOffSource::capture(image, budget, cancelled)?;
        let evidence = Evidence::capture(proof.image(), table, field, budget, cancelled)?;
        Ok(Self { proof, evidence })
    }
}

pub(super) fn request(
    table: &str,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    if !crate::identifier::valid_name(table, budget, cancelled)?
        || !crate::identifier::valid_name(field, budget, cancelled)?
        || !table.is_ascii()
        || !field.is_ascii()
    {
        return Err(Error::Source);
    }
    Ok(())
}

impl Evidence {
    pub fn identity(&self) -> NativeFieldDropIdentity {
        NativeFieldDropIdentity::from_observed(self.observed.identity)
    }

    pub fn capture(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let observed = Observed::capture(image, table, field, budget, cancelled)?;
        let scope = image.catalog().catalog().scope();
        let checked = check_table(&mut image.reader(), scope, table, budget, cancelled)?;
        if checked.rows() != 0 || observed.schema.declared_rows() != 0 {
            return Err(Error::Source);
        }
        let definition =
            existing_fields::capture_field_addition(image, &checked, budget, cancelled)?;
        let mut columns = definition.borrowed_columns(budget, cancelled)?;
        let position = observed
            .schema
            .columns()
            .iter()
            .position(|column| column.number() == observed.identity.column_number())
            .ok_or(Error::Source)?;
        budget.charge(Resource::Steps, columns.len() as u64)?;
        // Discard the removed declaration; encode only surviving field policies.
        let _ = columns.remove(position);
        let order = properties::Order::capture(&observed.properties, budget, cancelled)?;
        // The genuine encoder's exact-order check refuses orphan dictionary keys
        // and a missing remaining group; neither case is in the executed evidence.
        let body = properties::encode_body(&columns, order, budget, cancelled)
            .map_err(Error::from)?
            .ok_or(Error::Source)?;
        let mut removed = None;
        framing::ordered_group_blocks(&observed.properties, budget, cancelled, |group, span| {
            if group == observed.group && removed.replace(span).is_some() {
                return Err(crate::InspectError::Malformed);
            }
            Ok(())
        })?;
        let removed = removed.ok_or(Error::Source)?;
        let original = observed.properties.raw_bytes();
        budget.charge(Resource::Steps, (original.len() * 2) as u64)?;
        if body.len() != original.len() - removed.len()
            || body[..removed.start] != original[..removed.start]
            || body[removed.start..] != original[removed.end..]
        {
            return Err(Error::Preservation);
        }
        let catalog =
            inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
        let selected = column(&catalog)?;
        let origin = image
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == observed.schema.object_id())
            .ok_or(Error::Source)?;
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let descriptor = descriptor::capture(&mut reader, &catalog, selected, origin)?
            .absolute_range()?
            .ok_or(Error::Source)?;
        Ok(Self {
            observed,
            descriptor,
            body,
        })
    }
}
