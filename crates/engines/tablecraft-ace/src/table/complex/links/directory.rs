//! Complete key/ROW membership, deliberately separate from child payload results.
pub(in crate::table::complex) mod payload;

use super::{membership::Membership, read::read_parents};
use crate::{
    ComplexColumnSchema, ComplexParentLink, ComplexTableSchema, InspectError, NativeReadSource,
    NativeRowReference, TableSchema, TableSnapshotLimits, header::check, table::page,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::Column;

pub(in crate::table::complex) struct ChildKey {
    pub reference: NativeRowReference,
    pub foreign_key: i32,
    pub identity: i32,
}

pub(in crate::table::complex) struct Directory {
    pub schema: ComplexTableSchema,
    pub position: usize,
    field_position: usize,
    pub membership: Membership,
    pub children: Vec<ChildKey>,
    pub keys: Projection,
}

pub(in crate::table::complex) struct Projection {
    pub positions: Vec<usize>,
    pub columns: Vec<Column>,
    pub roles: Vec<usize>,
}

impl Projection {
    pub fn capture(
        declaration: &ComplexColumnSchema,
        roles: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let schema = declaration.flat_table();
        let all = super::super::references::columns(declaration, schema, budget, cancelled)?;
        budget.charge_allocation(all.len() as u64, size_of::<usize>() as u64)?;
        budget.charge_allocation(all.len() as u64, size_of::<Column>() as u64)?;
        let mut positions = Vec::new();
        let mut columns = Vec::new();
        positions
            .try_reserve_exact(all.len())
            .map_err(|_| InspectError::Allocation)?;
        columns
            .try_reserve_exact(all.len())
            .map_err(|_| InspectError::Allocation)?;
        for (position, column) in all.into_iter().enumerate() {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, roles.len() as u64)?;
            if roles.contains(&position) {
                positions.push(position);
                columns.push(column);
            }
        }
        Self::from_columns(positions, columns, roles, budget, cancelled)
    }

    /// Map actual owner-qualified projected carriers to their native roles.
    /// This does not qualify metadata or grant scalar-read permission.
    pub fn from_columns(
        positions: Vec<usize>,
        columns: Vec<Column>,
        roles: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        if positions.len() != columns.len() {
            return Err(InspectError::Malformed);
        }
        let mut previous = None;
        for position in &positions {
            check(budget, cancelled)?;
            if previous.is_some_and(|prior| prior >= *position) {
                return Err(InspectError::Malformed);
            }
            previous = Some(*position);
        }
        budget.charge_allocation(roles.len() as u64, size_of::<usize>() as u64)?;
        let mut projected = Vec::new();
        projected
            .try_reserve_exact(roles.len())
            .map_err(|_| InspectError::Allocation)?;
        for role in roles {
            let mut found = None;
            for (ordinal, position) in positions.iter().enumerate() {
                check(budget, cancelled)?;
                if position == role {
                    found = Some(ordinal);
                }
            }
            projected.push(found.ok_or(InspectError::Malformed)?);
        }
        Ok(Self {
            positions,
            columns,
            roles: projected,
        })
    }
}

impl Directory {
    pub fn capture<R: NativeReadSource>(
        source: &mut R,
        schema: ComplexTableSchema,
        field_position: usize,
        keys: Projection,
        limits: TableSnapshotLimits,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let (position, parents) =
            read_parents(source, &schema, field_position, limits, budget, cancelled)?;
        let declaration = &schema.columns()[position];
        let flat = declaration.flat_table();
        let roles = reference_positions(declaration, flat, budget, cancelled)?;
        if keys.positions.len() != roles.len() || keys.roles.len() != roles.len() {
            return Err(InspectError::Malformed);
        }
        for (role, expected) in roles.iter().enumerate() {
            check(budget, cancelled)?;
            let projected = *keys.roles.get(role).ok_or(InspectError::Malformed)?;
            let field = flat
                .columns()
                .get(*expected)
                .ok_or(InspectError::Malformed)?;
            let carrier = keys.columns.get(projected).ok_or(InspectError::Malformed)?;
            let work = field
                .name()
                .len()
                .checked_add(carrier.name().len())
                .ok_or(InspectError::StructureLimit)?;
            budget.charge(Resource::Steps, work as u64)?;
            if keys.positions.get(projected) != Some(expected)
                || carrier.data_type() != tablecraft_schema::ScalarType::Int32
                || carrier.name() != field.name()
            {
                return Err(InspectError::Malformed);
            }
        }
        let rows = page::read_records_batched(
            source,
            flat,
            &keys.columns,
            Some(&keys.positions),
            limits,
            budget,
            cancelled,
        )?;
        let membership = parents.link_rows(
            rows.iter().map(|(_, row)| row.as_slice()),
            keys.roles[0],
            keys.roles[1],
            budget,
            cancelled,
        )?;

        budget.charge_allocation(rows.len() as u64, size_of::<ChildKey>() as u64)?;
        let mut children = Vec::new();
        children
            .try_reserve_exact(rows.len())
            .map_err(|_| InspectError::Allocation)?;
        let mut previous = None;
        for (reference, row) in rows {
            check(budget, cancelled)?;
            if previous.is_some_and(|old| old >= reference) {
                return Err(InspectError::Malformed);
            }
            previous = Some(reference);
            let (Some(Value::Int32(foreign_key)), Some(Value::Int32(identity))) =
                (row.get(keys.roles[0]), row.get(keys.roles[1]))
            else {
                // The actual shared membership pass above already validated
                // these carriers, NULL rules, uniqueness and parent bindings.
                return Err(InspectError::Malformed);
            };
            children.push(ChildKey {
                reference,
                foreign_key: *foreign_key,
                identity: *identity,
            });
        }
        check(budget, cancelled)?;
        Ok(Self {
            schema,
            position,
            field_position,
            membership,
            children,
            keys,
        })
    }

    pub fn column(&self) -> &ComplexColumnSchema {
        &self.schema.columns()[self.position]
    }

    pub fn parents(&self) -> &[ComplexParentLink] {
        self.membership.parents()
    }

    pub fn require_source<R: NativeReadSource>(
        &self,
        source: &mut R,
        scope: CatalogScope,
        limits: TableSnapshotLimits,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ComplexTableSchema, InspectError> {
        let observed = super::super::inspect_complex_schema(
            source,
            scope,
            self.schema.owner().name(),
            budget,
            cancelled,
        )?;
        require_definition(self.schema.owner(), observed.owner(), budget, cancelled)?;
        let mut selected = None;
        for column in observed.columns() {
            check(budget, cancelled)?;
            if column.number() == self.column().number() {
                selected = Some(column);
            }
        }
        let selected = selected.ok_or(InspectError::Malformed)?;
        let expected = self.column();
        if selected.type_id() != expected.type_id()
            || selected.foreign_key_number() != expected.foreign_key_number()
            || selected.identity_number() != expected.identity_number()
        {
            return Err(InspectError::Malformed);
        }
        require_definition(
            expected.type_table(),
            selected.type_table(),
            budget,
            cancelled,
        )?;
        require_definition(
            expected.flat_table(),
            selected.flat_table(),
            budget,
            cancelled,
        )?;
        let (position, parents) = read_parents(
            source,
            &self.schema,
            self.field_position,
            limits,
            budget,
            cancelled,
        )?;
        if position != self.position {
            return Err(InspectError::Malformed);
        }
        parents.require_keys(self.parents(), budget, cancelled)?;
        Ok(observed)
    }
}

pub(in crate::table::complex) fn reference_positions(
    column: &ComplexColumnSchema,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[usize; 2], InspectError> {
    let mut result = [None; 2];
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == column.foreign_key_number() {
            result[0] = Some(position);
        }
        if field.number() == column.identity_number() {
            result[1] = Some(position);
        }
    }
    Ok([
        result[0].ok_or(InspectError::Malformed)?,
        result[1].ok_or(InspectError::Malformed)?,
    ])
}

fn require_definition(
    expected: &TableSchema,
    observed: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    // same_definition walks column declarations; pay its entire bound before
    // comparing, including paths that reject an early changed definition.
    let mut work = (size_of_val(expected.columns()) as u64)
        .checked_add(size_of_val(observed.columns()) as u64)
        .ok_or(InspectError::StructureLimit)?;
    for schema in [expected, observed] {
        check(budget, cancelled)?;
        work = work
            .checked_add(size_of_val(schema.layout.fields.as_slice()) as u64)
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
    budget.charge(Resource::Steps, work)?;
    if !expected.same_definition(observed) {
        return Err(InspectError::Malformed);
    }
    check(budget, cancelled)
}
