//! Select existing typed row codecs; no mixed-specific key concatenation exists.
use crate::format::{
    data_type::{SHORT_BINARY, SHORT_TEXT},
    reader::Reader,
};
use crate::table::indexes::{
    binary::{self, BinaryIndexSelection, BinaryProfile},
    composite::{self, DecimalColumns, DecimalValue, MixedValue},
    decimal::{self, DecimalIndexSelection, DecimalProfile, DecimalType},
    encoded_tuple::{self, EncodedTupleProfile},
    guid::{self, GuidIndexSelection, GuidProfile},
    mutation::{
        IndexDataPatch, NumericKey,
        composite::TupleProfile,
        encoding,
        profile::SingleProfile,
        replacement::{RowKeyUpdate, edit},
    },
    profile::Coverage,
    text::{self, TextProfile},
};
use crate::{EditError, InspectError, NativeImage, NativeIndex, TableSchema};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

enum Profile<'a> {
    Text(TextProfile),
    Binary(BinaryProfile<'a>),
    Numeric(SingleProfile),
    Guid(GuidProfile),
    Decimal(DecimalProfile),
    Tuple(TupleProfile),
    GuidTuple(TupleProfile<MixedValue>),
    DecimalTuple(TupleProfile<DecimalValue, DecimalColumns>),
    EncodedTuple(EncodedTupleProfile<'a>),
}

fn profile<'a>(
    index: &NativeIndex,
    schema: &'a TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Profile<'a>, EditError> {
    if index.columns().len() == 1 {
        let key = index.columns()[0];
        let column = schema
            .columns()
            .iter()
            .position(|field| field.number() == key.number())
            .ok_or(InspectError::Malformed)?;
        let field = &schema.columns()[column];
        return match field.native_type() {
            SHORT_TEXT => Ok(Profile::Text(TextProfile::new(
                column,
                field,
                key.descending(),
            )?)),
            SHORT_BINARY => Ok(Profile::Binary(BinaryProfile {
                column,
                descending: key.descending(),
                maximum: field.storage_length(),
                selection: BinaryIndexSelection::All,
            })),
            _ => match field.scalar_type().ok_or(InspectError::Unsupported)? {
                ScalarType::Guid => Ok(Profile::Guid(GuidProfile {
                    column,
                    descending: key.descending(),
                    selection: GuidIndexSelection::All,
                })),
                ScalarType::Decimal => Ok(Profile::Decimal(DecimalProfile {
                    column,
                    descending: key.descending(),
                    kind: DecimalType::from_column(field)?,
                    selection: DecimalIndexSelection::All,
                })),
                kind => Ok(Profile::Numeric(SingleProfile {
                    column,
                    descending: key.descending(),
                    kind: NumericKey::new(kind)?,
                })),
            },
        };
    }

    if encoded_tuple::has_variable(index, schema)? {
        return Ok(Profile::EncodedTuple(EncodedTupleProfile::new(
            index,
            schema,
            Coverage::CompleteTable,
        )?));
    }
    if composite::has_decimal(index, schema)? {
        return TupleProfile::<DecimalValue, DecimalColumns>::new_decimal(
            index, schema, budget, cancelled,
        )
        .map(Profile::DecimalTuple);
    }
    let has_guid = index.columns().iter().any(|key| {
        schema.columns().iter().any(|field| {
            field.number() == key.number() && field.scalar_type() == Some(ScalarType::Guid)
        })
    });
    if has_guid {
        TupleProfile::<MixedValue>::new(index, schema, budget, cancelled).map(Profile::GuidTuple)
    } else {
        TupleProfile::<i64>::new(index, schema, budget, cancelled).map(Profile::Tuple)
    }
}

pub(super) fn qualify(
    index: &NativeIndex,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // Charge source column-resolution scans before constructing the existing
    // profile. Typed tuple constructors also retain their own metered checks.
    let work = index
        .columns()
        .len()
        .checked_mul(schema.columns().len())
        .and_then(|work| work.checked_mul(3))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    profile(index, schema, budget, cancelled)?;
    Ok(())
}

pub(super) fn replace(
    image: &NativeImage,
    schema: &TableSchema,
    index: &NativeIndex,
    updates: &[RowKeyUpdate<'_>],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    reader.work(3 * index.columns().len() * schema.columns().len())?;
    let profile = profile(index, schema, reader.budget, reader.cancelled)?;
    let owner = schema.definition_page();
    match profile {
        Profile::Text(profile) => edit::<_, { text::MAX_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::Binary(profile) => edit::<_, { binary::MAX_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::Numeric(profile) => edit::<_, { encoding::MAX_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::Guid(profile) => edit::<_, { guid::MAX_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::Decimal(profile) => edit::<_, { decimal::MAX_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::Tuple(profile) => edit::<_, { composite::MAX_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::GuidTuple(profile) => edit::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::DecimalTuple(profile) => edit::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
        Profile::EncodedTuple(profile) => edit::<_, { encoded_tuple::MAX_ENTRY_BYTES }>(
            image, reader, owner, index, profile, updates, patches,
        ),
    }
}
