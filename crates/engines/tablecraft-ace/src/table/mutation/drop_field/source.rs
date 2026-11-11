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

const COPY_BYTES: usize = 4096;

#[derive(Clone, Copy)]
pub(super) enum Purpose {
    SettledEmpty,
    CurrentTail,
    RetainedTail,
    RetainedTypedTail,
    RetainedNonTail,
}

impl Purpose {
    pub(super) fn retained(self) -> bool {
        matches!(
            self,
            Self::RetainedTail | Self::RetainedTypedTail | Self::RetainedNonTail
        )
    }
}

pub(super) struct Source {
    pub proof: FieldHistoryOffSource,
    pub evidence: Evidence,
}

/// Metadata alone is not mutation authority; Source retains the consumed proof.
pub(super) struct Evidence {
    pub purpose: Purpose,
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
        Self::capture_for(
            image,
            table,
            field,
            Purpose::SettledEmpty,
            budget,
            cancelled,
        )
    }

    pub(super) fn capture_current_tail(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(image, table, field, Purpose::CurrentTail, budget, cancelled)
    }

    pub(super) fn capture_retained_tail(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::RetainedTail,
            budget,
            cancelled,
        )
    }

    pub(super) fn capture_retained_typed_tail(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::RetainedTypedTail,
            budget,
            cancelled,
        )
    }

    pub(super) fn capture_retained_non_tail(
        image: &NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        Self::capture_for(
            image,
            table,
            field,
            Purpose::RetainedNonTail,
            budget,
            cancelled,
        )
    }

    fn capture_for(
        image: &NativeImage,
        table: &str,
        field: &str,
        purpose: Purpose,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let observed = if matches!(purpose, Purpose::RetainedNonTail) {
            Observed::capture_retained_non_tail(image, table, field, budget, cancelled)?
        } else if matches!(purpose, Purpose::RetainedTypedTail) {
            Observed::capture_retained_typed_tail(image, table, field, budget, cancelled)?
        } else if matches!(purpose, Purpose::RetainedTail) {
            Observed::capture_retained_schema_drop(image, table, field, budget, cancelled)?
        } else if matches!(purpose, Purpose::CurrentTail) {
            Observed::capture_current_tail(image, table, field, budget, cancelled)?
        } else {
            Observed::capture(image, table, field, budget, cancelled)?
        };
        let scope = image.catalog().catalog().scope();
        let checked = check_table(&mut image.reader(), scope, table, budget, cancelled)?;
        let invalid_rows = match purpose {
            Purpose::CurrentTail => {
                checked.rows() == 0 || checked.rows() != observed.schema.declared_rows() as usize
            }
            Purpose::RetainedTail | Purpose::RetainedTypedTail | Purpose::RetainedNonTail => {
                checked.rows() != observed.schema.declared_rows() as usize
            }
            Purpose::SettledEmpty => checked.rows() != 0 || observed.schema.declared_rows() != 0,
        };
        if invalid_rows {
            return Err(Error::Source);
        }
        let body = if matches!(
            purpose,
            Purpose::CurrentTail
                | Purpose::RetainedTail
                | Purpose::RetainedTypedTail
                | Purpose::RetainedNonTail
        ) {
            current_body(&observed, budget, cancelled)?
        } else {
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
            let _ = columns.remove(position);
            let order = properties::Order::capture(&observed.properties, budget, cancelled)?;
            properties::encode_body(&columns, order, budget, cancelled)
                .map_err(Error::from)?
                .ok_or(Error::Source)?
        };
        let mut removed = None;
        let visit = |group, span| {
            if group == observed.group && removed.replace(span).is_some() {
                return Err(crate::InspectError::Malformed);
            }
            Ok(())
        };
        match purpose {
            Purpose::SettledEmpty => {
                framing::ordered_group_blocks(&observed.properties, budget, cancelled, visit)?
            }
            Purpose::CurrentTail
            | Purpose::RetainedTail
            | Purpose::RetainedTypedTail
            | Purpose::RetainedNonTail => framing::current_schema_group_blocks(
                &observed.properties,
                budget,
                cancelled,
                visit,
            )?,
        };
        let removed = removed.ok_or(Error::Source)?;
        let original = observed.properties.raw_bytes();
        budget.charge(Resource::Steps, (original.len() * 2) as u64)?;
        if body.len() != original.len() - removed.len()
            || body[..removed.start] != original[..removed.start]
            || body[removed.start..] != original[removed.end..]
        {
            return Err(Error::Preservation);
        }
        let descriptor = if purpose.retained() {
            observed.property_descriptor.start..observed.property_descriptor.end
        } else {
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
            descriptor::capture(&mut reader, &catalog, selected, origin)?
                .absolute_range()?
                .ok_or(Error::Source)?
        };
        Ok(Self {
            purpose,
            observed,
            descriptor,
            body,
        })
    }
}

fn current_body(
    observed: &Observed,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    for group in observed.properties.groups() {
        check(budget, cancelled)?;
        if group.kind() == crate::PropertyGroupKind::Column {
            let mut owners = 0;
            for field in observed.schema.columns() {
                check(budget, cancelled)?;
                budget.charge(
                    Resource::Steps,
                    group.name().len().max(field.name().len()) as u64,
                )?;
                owners += usize::from(group.name() == field.name());
            }
            if owners != 1 {
                return Err(Error::Source);
            }
        }
    }
    let mut removed = None;
    framing::current_schema_group_blocks(
        &observed.properties,
        budget,
        cancelled,
        |group, span| {
            if group == observed.group && removed.replace(span).is_some() {
                return Err(crate::InspectError::Malformed);
            }
            Ok(())
        },
    )?;
    let removed = removed.ok_or(Error::Source)?;
    let raw = observed.properties.raw_bytes();
    let length = raw.len().checked_sub(removed.len()).ok_or(Error::Source)?;
    budget.charge_allocation(length as u64, 1)?;
    let mut body = Vec::new();
    body.try_reserve_exact(length)
        .map_err(|_| crate::InspectError::Allocation)?;
    for part in [&raw[..removed.start], &raw[removed.end..]] {
        for block in part.chunks(COPY_BYTES) {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, block.len() as u64)?;
            body.extend_from_slice(block);
        }
    }
    Ok(body)
}
