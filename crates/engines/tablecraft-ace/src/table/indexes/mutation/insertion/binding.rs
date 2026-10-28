//! INSERT builds complete row keys; it does not implement UPDATE replacement.
use super::InsertRow;
use crate::{
    EditError, InspectError, NativeIndex, TableSchema,
    format::{
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    table::{
        indexes::{
            key::KeyWork,
            mutation::{
                profile::WriteProfile,
                retained::{Preserved, RetainedProfile},
                row_key::RowKeyProfile,
            },
            page::KeyRef,
        },
        record,
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::Resource, values::Value};

pub(in crate::table::indexes::mutation) trait InsertProfile:
    RetainedProfile
{
    fn request_key(
        &self,
        row: &InsertRow<'_>,
        work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError>;

    fn bind_source(
        &self,
        schema: &TableSchema,
        index: &NativeIndex,
        keys: &mut [KeyRef<Self::Key>],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError>;
}

impl<P: WriteProfile> InsertProfile for P {
    fn request_key(
        &self,
        row: &InsertRow<'_>,
        work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
        // Preserve the established fixed-profile validation and charging path.
        let wire = self.encoded_row(row, work)?;
        let (key, consumed) = self.decode(&wire, work)?;
        if consumed != wire.len() {
            return Err(InspectError::Malformed);
        }
        Ok(key)
    }

    fn bind_source(
        &self,
        _schema: &TableSchema,
        _index: &NativeIndex,
        _keys: &mut [KeyRef<Self::Key>],
        _reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        Ok(())
    }
}

impl<P: RowKeyProfile> InsertProfile for Preserved<P> {
    fn request_key(
        &self,
        row: &InsertRow<'_>,
        work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
        self.row_key_with_wire(row, work)
    }

    fn bind_source(
        &self,
        schema: &TableSchema,
        index: &NativeIndex,
        keys: &mut [KeyRef<Self::Key>],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let mut columns = Vec::new();
        reader
            .budget
            .charge_allocation(index.columns().len() as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
        columns
            .try_reserve_exact(index.columns().len())
            .map_err(|_| InspectError::Allocation)?;
        for key in index.columns() {
            let mut selected = None;
            for (position, field) in schema.columns().iter().enumerate() {
                reader.tick()?;
                if field.number() == key.number() && selected.replace(position).is_some() {
                    return Err(EditError::Preservation);
                }
            }
            columns.push(selected.ok_or(EditError::Preservation)?);
        }
        reader.work(columns.len() * columns.len())?;
        columns.sort_unstable();
        columns.dedup();
        let mut current = None;
        for entry in keys {
            reader.tick()?;
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let number = reference_page(entry.row);
            if !matches!(&current, Some((page, _)) if *page == number) {
                current = Some((number, reader.page(number)?));
            }
            let (_, page) = current.as_ref().ok_or(EditError::Preservation)?;
            let raw = reader.row_from_page(
                number,
                reference_slot(entry.row),
                schema.definition_page(),
                page,
            )?;
            let selected = record::decode_selected(&raw, schema, Some(&columns), reader)?;
            if selected.len() != columns.len() {
                return Err(EditError::Preservation);
            }
            let mut values = Vec::new();
            reader
                .budget
                .charge_allocation(schema.columns().len() as u64, size_of::<Value>() as u64)
                .map_err(InspectError::from)?;
            values
                .try_reserve_exact(schema.columns().len())
                .map_err(|_| InspectError::Allocation)?;
            values.resize_with(schema.columns().len(), || Value::Null);
            for (column, value) in columns.iter().zip(selected) {
                reader.tick()?;
                values[*column] = value;
            }
            entry.key = self.bind_row(
                &entry.key,
                &values,
                &mut KeyWork {
                    budget: reader.budget,
                    cancelled: reader.cancelled,
                },
            )?;
        }
        Ok(())
    }
}
