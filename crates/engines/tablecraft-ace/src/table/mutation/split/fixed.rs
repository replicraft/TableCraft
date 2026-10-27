//! Closed typed integral/Currency facade over the shared split proof and staging.
use super::super::{EditError, FixedScalarUpdate, WriteBlocker, fixed, physical, rows};
use super::{
    Staged,
    binding::{Binding, Scope},
    finish, preflight, stage_checked,
};
use crate::{
    Ace16WritePreview, InspectError, NativeImage, TableCheck, TableSchema, format::reader::Reader,
    table::indexes::split_scalar_supported,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) struct FixedOperation<'a> {
    pub table: &'a str,
    pub column: &'a str,
    pub requests: &'a [FixedScalarUpdate],
}

impl NativeImage {
    /// Preview non-NULL Byte, Int16/32/64 or Currency updates with index splits.
    ///
    /// Original request carriers must exactly match the native fixed field.
    /// Existing NULL peers retain their masks/payloads and indexed membership;
    /// NULL requests and new Boolean/float/DATE/GUID/Decimal/composite allocation
    /// are not admitted. Qualified composite indexes may still be maintained when
    /// their existing pages fit. All original gates and final constraints apply.
    ///
    /// This feature-gated engineering candidate is non-journalable and does not
    /// widen ordinary saves or the existing fixed/page/variable preview methods.
    pub fn preview_ace16_split_fixed_updates(
        self,
        table: &str,
        column: &str,
        requests: &[FixedScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.split_fixed",
            assignments = requests.len()
        )
        .entered();
        let operation = FixedOperation {
            table,
            column,
            requests,
        };
        let structural = preflight(&self, &operation, budget, cancelled)?;
        let kind = selected_kind(structural.schema(), column)?;
        match kind {
            ScalarType::Byte => {
                generate::<{ size_of::<u8>() }>(self, &operation, structural, budget, cancelled)
            }
            ScalarType::Int16 => {
                generate::<{ size_of::<i16>() }>(self, &operation, structural, budget, cancelled)
            }
            ScalarType::Int32 => {
                generate::<{ size_of::<i32>() }>(self, &operation, structural, budget, cancelled)
            }
            ScalarType::Int64 | ScalarType::Currency => {
                generate::<{ size_of::<i64>() }>(self, &operation, structural, budget, cancelled)
            }
            _ => Err(EditError::Unsupported(WriteBlocker::Column)),
        }
    }
}

fn generate<const WIDTH: usize>(
    image: NativeImage,
    operation: &FixedOperation<'_>,
    structural: TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let staged: Staged<WIDTH> = stage_checked(&image, operation, structural, budget, cancelled)?;
    finish::bound(image, operation, &staged, budget, cancelled)
}

fn selected_kind(schema: &TableSchema, column: &str) -> Result<ScalarType, EditError> {
    let kind = schema
        .columns()
        .iter()
        .find(|field| field.name().eq_ignore_ascii_case(column))
        .and_then(|field| field.scalar_type())
        .filter(|kind| split_scalar_supported(*kind))
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    Ok(kind)
}

impl Scope for FixedOperation<'_> {
    fn table(&self) -> &str {
        self.table
    }

    fn column(&self) -> &str {
        self.column
    }

    fn request_count(&self) -> usize {
        self.requests.len()
    }

    fn unique(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        rows::unique(self.requests, budget, cancelled)
    }
}

impl<const WIDTH: usize> Binding<WIDTH> for FixedOperation<'_> {
    fn kind(&self, schema: &TableSchema) -> Result<ScalarType, EditError> {
        selected_kind(schema, self.column)
    }

    fn patches(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        schema: &TableSchema,
        position: usize,
    ) -> Result<Vec<physical::Patch<WIDTH>>, EditError> {
        let kind = selected_kind(schema, self.column)?;
        reader
            .budget
            .charge_allocation(
                self.requests.len() as u64,
                size_of::<fixed::EncodedUpdate<WIDTH>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(self.requests.len())
            .map_err(|_| InspectError::Allocation)?;
        for request in self.requests {
            reader.tick()?;
            encoded.push(fixed::EncodedUpdate {
                row: request.row(),
                expected: fixed::encode_fixed(kind, request.expected())?,
                value: fixed::encode_fixed(kind, request.value())?,
            });
        }
        physical::patches(reader, schema, position, &encoded)
    }
}
