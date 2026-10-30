//! Hold each actual typed final key population before the first INDEX reserve.
use super::super::super::{
    replacement::{
        keys,
        mixed::profiles::{self, Profile},
    },
    row_key::RowKeyProfile,
};
use super::super::{Plan, edit_prepared, nonroot, source};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema,
    format::reader::Reader,
    table::indexes::{
        binary,
        composite::{self, DecimalColumns, DecimalValue, MixedValue},
        decimal, encoded_tuple, guid,
        mutation::{
            composite::TupleProfile, encoding, profile::SingleProfile, replacement::RowKeyUpdate,
        },
        text,
    },
};
use std::io::Cursor;

pub(super) struct Typed<'a, P: RowKeyProfile> {
    index: &'a NativeIndex,
    first_column: usize,
    keys: keys::Prepared<P>,
}

#[allow(
    clippy::large_enum_variant,
    reason = "Profiles live in the same fallibly reserved, metered owner vector; separate infallible boxes would add unmetered allocations"
)]
pub(super) enum Prepared<'a> {
    Text(Typed<'a, text::TextProfile>),
    Binary(Typed<'a, binary::BinaryProfile<'a>>),
    Numeric(Typed<'a, SingleProfile>),
    Guid(Typed<'a, guid::GuidProfile>),
    Decimal(Typed<'a, decimal::DecimalProfile>),
    Tuple(Typed<'a, TupleProfile>),
    GuidTuple(Typed<'a, TupleProfile<MixedValue>>),
    DecimalTuple(Typed<'a, TupleProfile<DecimalValue, DecimalColumns>>),
    EncodedTuple(Typed<'a, encoded_tuple::EncodedTupleProfile<'a>>),
}

fn prepare_typed<'a, P: RowKeyProfile, const N: usize>(
    image: &NativeImage,
    schema: &TableSchema,
    owner: &source::Physical<'a>,
    updates: &[RowKeyUpdate<'_>],
    profile: P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Typed<'a, P>, EditError> {
    let keys = keys::prepare::<P, N>(
        image,
        reader,
        schema.definition_page(),
        owner.index,
        profile,
        updates,
    )?;
    source::require_tree(&keys.tree, &owner.pages, reader)?;
    reader.work(schema.columns().len())?;
    let first = owner
        .index
        .columns()
        .first()
        .ok_or(InspectError::Malformed)?;
    let first_column = schema
        .columns()
        .iter()
        .position(|field| field.number() == first.number())
        .ok_or(InspectError::Malformed)?;
    Ok(Typed {
        index: owner.index,
        first_column,
        keys,
    })
}

pub(super) fn prepare<'a>(
    image: &NativeImage,
    schema: &'a TableSchema,
    owner: &source::Physical<'a>,
    updates: &[RowKeyUpdate<'_>],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Prepared<'a>, EditError> {
    let comparisons = owner
        .index
        .columns()
        .len()
        .checked_mul(schema.columns().len())
        .and_then(|count| count.checked_mul(profiles::COLUMN_CLASSIFICATION_PASSES))
        .ok_or(InspectError::Malformed)?;
    reader.work(comparisons)?;
    let profile = profiles::profile(owner.index, schema, reader.budget, reader.cancelled)?;
    macro_rules! prepare {
        ($variant:ident, $profile:expr, $maximum:expr) => {
            prepare_typed::<_, { $maximum }>(image, schema, owner, updates, $profile, reader)
                .map(Prepared::$variant)
        };
    }
    match profile {
        Profile::Text(profile) => prepare!(Text, profile, text::MAX_ENTRY_BYTES),
        Profile::Binary(profile) => prepare!(Binary, profile, binary::MAX_ENTRY_BYTES),
        Profile::Numeric(profile) => prepare!(Numeric, profile, encoding::MAX_ENTRY_BYTES),
        Profile::Guid(profile) => prepare!(Guid, profile, guid::MAX_ENTRY_BYTES),
        Profile::Decimal(profile) => prepare!(Decimal, profile, decimal::MAX_ENTRY_BYTES),
        Profile::Tuple(profile) => prepare!(Tuple, profile, composite::MAX_ENTRY_BYTES),
        Profile::GuidTuple(profile) => {
            prepare!(GuidTuple, profile, composite::MAX_GUID_ENTRY_BYTES)
        }
        Profile::DecimalTuple(profile) => {
            prepare!(DecimalTuple, profile, composite::MAX_GUID_ENTRY_BYTES)
        }
        Profile::EncodedTuple(profile) => {
            prepare!(EncodedTuple, profile, encoded_tuple::MAX_ENTRY_BYTES)
        }
    }
}

fn write_typed<P: RowKeyProfile, const N: usize>(
    typed: Typed<'_, P>,
    image: &NativeImage,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut patches = std::mem::take(&mut plan.patches);
    edit_prepared(
        &nonroot::Operation::<_, N> {
            image,
            index: typed.index,
            profile: &typed.keys.profile,
            keys: &typed.keys.entries,
        },
        &typed.keys.tree,
        typed.first_column,
        &mut patches,
        plan,
        reader,
    )?;
    plan.patches = patches;
    Ok(())
}

impl Prepared<'_> {
    pub(super) fn write(
        self,
        image: &NativeImage,
        plan: &mut Plan,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        match self {
            Self::Text(typed) => {
                write_typed::<_, { text::MAX_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::Binary(typed) => {
                write_typed::<_, { binary::MAX_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::Numeric(typed) => {
                write_typed::<_, { encoding::MAX_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::Guid(typed) => {
                write_typed::<_, { guid::MAX_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::Decimal(typed) => {
                write_typed::<_, { decimal::MAX_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::Tuple(typed) => {
                write_typed::<_, { composite::MAX_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::GuidTuple(typed) => {
                write_typed::<_, { composite::MAX_GUID_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::DecimalTuple(typed) => {
                write_typed::<_, { composite::MAX_GUID_ENTRY_BYTES }>(typed, image, plan, reader)
            }
            Self::EncodedTuple(typed) => {
                write_typed::<_, { encoded_tuple::MAX_ENTRY_BYTES }>(typed, image, plan, reader)
            }
        }
    }
}
