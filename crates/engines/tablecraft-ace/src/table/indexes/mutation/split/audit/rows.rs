//! Reconstruct joint final keys from independently certified original groups.
use super::super::super::{
    replacement::{
        keys,
        mixed::profiles::{self, Profile},
    },
    row_key::RowKeyProfile,
};
use super::super::{Plan, rows::Changes};
use super::retained;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema,
    format::reader::Reader,
    table::indexes::{binary, composite, decimal, encoded_tuple, guid, mutation::encoding, text},
};
use std::io::Cursor;

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub schema: &'a TableSchema,
    pub index: &'a NativeIndex,
    pub changes: &'a Changes<'a>,
}

pub(super) fn prove(
    operation: &Operation<'_>,
    plan: &Plan,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut retained::Progress<'_>,
) -> Result<retained::Disposition, EditError> {
    let work = operation
        .index
        .columns()
        .len()
        .checked_mul(operation.schema.columns().len())
        .and_then(|work| work.checked_mul(profiles::COLUMN_CLASSIFICATION_PASSES))
        .ok_or(InspectError::StructureLimit)?;
    actual.work(work)?;
    let profile = profiles::profile(
        operation.index,
        operation.schema,
        actual.budget,
        actual.cancelled,
    )?;
    match profile {
        Profile::Text(profile) => {
            prepared::<_, { text::MAX_ENTRY_BYTES }>(operation, profile, plan, actual, progress)
        }
        Profile::Binary(profile) => {
            prepared::<_, { binary::MAX_ENTRY_BYTES }>(operation, profile, plan, actual, progress)
        }
        Profile::Numeric(profile) => {
            prepared::<_, { encoding::MAX_ENTRY_BYTES }>(operation, profile, plan, actual, progress)
        }
        Profile::Guid(profile) => {
            prepared::<_, { guid::MAX_ENTRY_BYTES }>(operation, profile, plan, actual, progress)
        }
        Profile::Decimal(profile) => {
            prepared::<_, { decimal::MAX_ENTRY_BYTES }>(operation, profile, plan, actual, progress)
        }
        Profile::Tuple(profile) => prepared::<_, { composite::MAX_ENTRY_BYTES }>(
            operation, profile, plan, actual, progress,
        ),
        Profile::GuidTuple(profile) => prepared::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
            operation, profile, plan, actual, progress,
        ),
        Profile::DecimalTuple(profile) => prepared::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
            operation, profile, plan, actual, progress,
        ),
        Profile::EncodedTuple(profile) => prepared::<_, { encoded_tuple::MAX_ENTRY_BYTES }>(
            operation, profile, plan, actual, progress,
        ),
    }
}

fn prepared<P: RowKeyProfile, const N: usize>(
    operation: &Operation<'_>,
    original: P,
    plan: &Plan,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut retained::Progress<'_>,
) -> Result<retained::Disposition, EditError> {
    let mut source = operation.image.reader();
    let prepared = keys::prepare::<P, N>(
        operation.image,
        &mut Reader {
            source: &mut source,
            header: operation.image.catalog().header(),
            budget: actual.budget,
            cancelled: actual.cancelled,
            reads: 0,
        },
        operation.schema.definition_page(),
        operation.index,
        original,
        operation.changes.updates,
    )?;

    actual.work(operation.schema.columns().len())?;
    let first = operation
        .index
        .columns()
        .first()
        .ok_or(InspectError::Malformed)?;
    let column = operation
        .schema
        .columns()
        .iter()
        .position(|field| field.number() == first.number())
        .ok_or(InspectError::Malformed)?;

    retained::prove::<_, N>(
        &retained::Operation {
            image: operation.image,
            index: operation.index,
            owner: operation.schema.definition_page(),
            column,
            profile: &prepared.profile,
            tree: &prepared.tree,
            keys: &prepared.entries,
        },
        plan,
        actual,
        progress,
    )
}
