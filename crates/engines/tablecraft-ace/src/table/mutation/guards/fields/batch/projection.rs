//! Actual Int32 frame cells over an already certified scalar projection.
use super::super::super::{BeforeEventGuardError, BeforeGuardUnsupported, buffer};
use crate::{
    EditError, IndexInspection, InspectError, TableSnapshot, WriteBlocker,
    format::data_type::INT32, header::check, table::mutation::field,
};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

struct Field {
    native: usize,
    snapshot: usize,
    write_blocker: Option<WriteBlocker>,
}

/// No opaque/declaration-only field has a fabricated frame cell. This is not a
/// replacement TableSnapshot or the public complete BeforeChangeFinalRow model.
pub(in crate::table::mutation) struct Int32Projection<'source> {
    table: &'source TableSnapshot,
    fields: Vec<Field>,
}

impl<'source> Int32Projection<'source> {
    pub(in crate::table::mutation) fn capture(
        table: &'source TableSnapshot,
        indexes: &IndexInspection,
        dependencies: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, BeforeEventGuardError> {
        if table.schema().object_id() != indexes.schema().object_id()
            || table.schema().definition_page() != indexes.schema().definition_page()
        {
            return Err(BeforeEventGuardError::Preservation);
        }
        let mut fields = buffer::vector(table.columns().len(), budget, cancelled)?;
        for (native, field) in table.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, field.name().len() as u64)
                .map_err(InspectError::from)?;
            if field.name().eq_ignore_ascii_case("IsInsert") {
                return Err(unsupported());
            }
            // Read eligibility is storage-shaped, not write permission. Keep
            // ordinary counters/indexed operands, but never expose complex FKs.
            if field.native_type() != INT32
                || field.scalar_type() != Some(ScalarType::Int32)
                || !field.fixed()
                || field.storage_length() != size_of::<i32>()
                || field.extended_flags() != 0
            {
                continue;
            }

            let mut matched = None;
            for (snapshot, column) in table.columns().iter().enumerate() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        (column.name().len() + field.name().len()) as u64,
                    )
                    .map_err(InspectError::from)?;
                if column.name().eq_ignore_ascii_case(field.name())
                    && (matched.replace(snapshot).is_some()
                        || column.data_type() != ScalarType::Int32)
                {
                    return Err(InspectError::Malformed.into());
                }
            }
            let mut write_blocker = match field::fixed_column_at(
                table.schema(),
                native,
                ScalarType::Int32,
                size_of::<i32>(),
            ) {
                Ok(_) => None,
                Err(EditError::Unsupported(blocker)) => Some(blocker),
                Err(error) => return Err(BeforeEventGuardError::Intent(error)),
            };
            for dependency in dependencies {
                check(budget, cancelled)?;
                if *dependency == native && write_blocker.is_none() {
                    write_blocker = Some(WriteBlocker::IndexedColumn);
                }
            }
            for index in indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    if key.number() == native && write_blocker.is_none() {
                        write_blocker = Some(WriteBlocker::IndexedColumn);
                    }
                }
            }
            fields.push(Field {
                native,
                snapshot: matched.ok_or(InspectError::Malformed)?,
                write_blocker,
            });
        }
        check(budget, cancelled)?;
        Ok(Self { table, fields })
    }

    pub(in crate::table::mutation::guards) fn len(&self) -> usize {
        self.fields.len()
    }

    pub(in crate::table::mutation::guards) fn name(
        &self,
        frame: usize,
    ) -> Result<&str, BeforeEventGuardError> {
        let field = self.fields.get(frame).ok_or(InspectError::Malformed)?;
        Ok(self.table.schema().columns()[field.native].name())
    }

    pub(in crate::table::mutation) fn native(
        &self,
        frame: usize,
    ) -> Result<usize, BeforeEventGuardError> {
        self.fields
            .get(frame)
            .map(|field| field.native)
            .ok_or(InspectError::Malformed.into())
    }

    pub(in crate::table::mutation) fn snapshot_position(
        &self,
        frame: usize,
    ) -> Result<usize, BeforeEventGuardError> {
        self.fields
            .get(frame)
            .map(|field| field.snapshot)
            .ok_or(InspectError::Malformed.into())
    }

    pub(in crate::table::mutation) fn frame_position(
        &self,
        native: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<usize>, BeforeEventGuardError> {
        for (position, field) in self.fields.iter().enumerate() {
            check(budget, cancelled)?;
            if field.native == native {
                return Ok(Some(position));
            }
        }
        Ok(None)
    }

    pub(in crate::table::mutation::guards) fn object_id(&self) -> ObjectId {
        self.table.schema().object_id()
    }

    pub(in crate::table::mutation) fn require_table(
        &self,
        table: &TableSnapshot,
    ) -> Result<(), BeforeEventGuardError> {
        if !std::ptr::eq(self.table, table) {
            return Err(BeforeEventGuardError::Preservation);
        }
        Ok(())
    }

    /// Every compiled SetField uses this check, including dormant definitions.
    /// Read access to an indexed operand never confers write permission.
    pub(in crate::table::mutation::guards) fn require_target(
        &self,
        frame: usize,
    ) -> Result<(), BeforeEventGuardError> {
        let field = self.fields.get(frame).ok_or(InspectError::Malformed)?;
        if let Some(blocker) = field.write_blocker {
            return Err(BeforeEventGuardError::Intent(EditError::Unsupported(
                blocker,
            )));
        }
        Ok(())
    }

    /// Copies only immediate Int32/NULL carriers, with a paid bounded allocation.
    /// Original and proposed copies are made separately by the row-frame owner.
    pub(in crate::table::mutation) fn copy_row(
        &self,
        ordinal: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Value>, BeforeEventGuardError> {
        let row = self
            .table
            .rows()
            .get(ordinal)
            .ok_or(InspectError::Malformed)?;
        let mut values = buffer::vector(self.fields.len(), budget, cancelled)?;
        for field in &self.fields {
            check(budget, cancelled)?;
            values.push(
                match row.get(field.snapshot).ok_or(InspectError::Malformed)? {
                    Value::Null => Value::Null,
                    Value::Int32(value) => Value::Int32(*value),
                    _ => return Err(InspectError::Malformed.into()),
                },
            );
        }
        Ok(values)
    }
}

fn unsupported() -> BeforeEventGuardError {
    BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::SourceProfile)
}
