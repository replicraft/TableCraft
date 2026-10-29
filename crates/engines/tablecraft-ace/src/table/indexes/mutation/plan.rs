pub(super) mod keys;

use super::super::{IndexCheck, IndexCheckStatus, NativeIndex, composite};
use super::super::{
    composite::{DecimalColumns, DecimalValue, MixedValue},
    decimal::{self, DecimalIndexSelection, DecimalProfile, DecimalType},
    guid::{self, GuidProfile},
};
#[cfg(feature = "ace16-write-preview")]
use super::{
    super::{binary, text},
    retained::Preserved,
};
use super::{
    IndexDataPatch, IndexKeyUpdate, NumericKey, PatchSet,
    composite::TupleProfile,
    encoding,
    profile::{SingleProfile, WriteProfile},
    tree::Rewrite,
};
#[cfg(feature = "ace16-write-preview")]
use crate::format::data_type::{SHORT_BINARY, SHORT_TEXT};
use crate::{
    EditError, FileFormat, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::reader::Reader, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    patches_with(
        image,
        schema,
        indexes,
        Change::Update(updates),
        #[cfg(feature = "ace16-write-preview")]
        None,
        budget,
        cancelled,
    )
    .map(|patches| patches.indexes)
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) fn splitting_patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<super::split::Plan, EditError> {
    let mut plan = super::split::Plan::new(image, schema.definition);
    let patches = patches_with(
        image,
        schema,
        indexes,
        Change::Update(updates),
        Some(&mut plan),
        budget,
        cancelled,
    )?;
    plan.patches = patches.indexes;
    super::split::seal(image, indexes, &mut plan, budget, cancelled)?;
    Ok(plan)
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) fn delete_patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    references: &[u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PatchSet, EditError> {
    patches_with(
        image,
        schema,
        indexes,
        Change::Delete(references),
        None,
        budget,
        cancelled,
    )
}

#[derive(Clone, Copy)]
pub(super) enum Change<'a> {
    Update(&'a [IndexKeyUpdate]),
    #[cfg(feature = "ace16-write-preview")]
    Delete(&'a [u32]),
    #[cfg(feature = "ace16-write-preview")]
    DeleteUpdate(&'a [u32], &'a [IndexKeyUpdate]),
}

impl Change<'_> {
    fn len(self) -> usize {
        match self {
            Self::Update(updates) => updates.len(),
            #[cfg(feature = "ace16-write-preview")]
            Self::Delete(references) => references.len(),
            #[cfg(feature = "ace16-write-preview")]
            Self::DeleteUpdate(references, updates) => {
                references.len().saturating_add(updates.len())
            }
        }
    }

    fn permits_omissions(self) -> bool {
        match self {
            Self::Update(_) => false,
            #[cfg(feature = "ace16-write-preview")]
            Self::Delete(_) | Self::DeleteUpdate(_, _) => true,
        }
    }

    fn affects(self, index: &NativeIndex, schema: &TableSchema) -> bool {
        match self {
            Self::Update(updates) => updates.iter().any(|update| {
                index
                    .columns()
                    .iter()
                    .any(|key| key.number() == schema.columns[update.column].number())
            }),
            #[cfg(feature = "ace16-write-preview")]
            Self::Delete(_) | Self::DeleteUpdate(_, _) => true,
        }
    }
}

pub(super) fn patches_with(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    change: Change<'_>,
    #[cfg(feature = "ace16-write-preview")] mut splitting: Option<&mut super::split::Plan>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PatchSet, EditError> {
    check(budget, cancelled)?;
    // Format admission belongs to the public edit/preview entry points. This
    // private helper receives only already checked schemas/index coverage.
    let mut patches = PatchSet::default();
    // Every physical declaration must have a logical owner. Do not change a
    // live page also referenced by an uninspected/obsolete physical definition.
    // Even empty key updates must prove this: an unowned definition can hide an
    // affected column, and no-op requests must not imply complete write coverage.
    for number in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, indexes.len() as u64)
            .map_err(InspectError::from)?;
        if !indexes
            .iter()
            .any(|index| index.index().data_number() == number as u32)
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    if matches!(change, Change::Update(updates) if updates.is_empty()) {
        return Ok(patches);
    }
    budget
        .charge_allocation(indexes.len() as u64, size_of::<IndexDataPatch>() as u64)
        .map_err(InspectError::from)?;
    patches
        .indexes
        .try_reserve_exact(indexes.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for (position, check) in indexes.iter().enumerate() {
        reader.tick()?;
        let index = check.index();
        #[cfg(feature = "ace16-write-preview")]
        let change = match change {
            Change::DeleteUpdate(references, updates)
                if !Change::Update(updates).affects(index, schema) =>
            {
                Change::Delete(references)
            }
            other => other,
        };
        reader.work(change.len() * index.columns().len())?;
        if !change.affects(index, schema) {
            continue;
        }
        if check.status() != IndexCheckStatus::Checked
            || !index.constraints().is_some_and(|constraints| {
                !constraints.ignore_nulls() || change.permits_omissions()
            })
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        reader.work(indexes.len())?;
        if indexes.iter().any(|other| {
            other.index().data_number() != index.data_number()
                && other.index().root_page() == index.root_page()
        }) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if indexes[..position]
            .iter()
            .any(|previous| previous.index().data_number() == index.data_number())
        {
            continue;
        }
        if index.columns().len() == 1 {
            let column = schema
                .columns
                .iter()
                .position(|column| column.number() == index.columns()[0].number())
                .ok_or(InspectError::Malformed)?;
            let kind = schema.columns[column]
                .scalar_type()
                .ok_or(InspectError::Unsupported)?;
            #[cfg(feature = "ace16-write-preview")]
            if let Change::Delete(references) = change {
                let metadata = &schema.columns[column];
                if metadata.native_type() == SHORT_TEXT && kind == ScalarType::Text {
                    if metadata
                        .text_sort_order()
                        .and_then(|order| order.general_family())
                        .is_none()
                    {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                    let profile = Preserved(text::TextProfile::new(
                        column,
                        metadata,
                        index.columns()[0].descending(),
                    )?);
                    super::delete::edit::<_, { text::MAX_ENTRY_BYTES }>(
                        image,
                        &mut reader,
                        index,
                        schema.definition,
                        references,
                        &profile,
                        &mut patches,
                    )?;
                    continue;
                }
                if metadata.native_type() == SHORT_BINARY && kind == ScalarType::Binary {
                    let profile = Preserved(binary::BinaryProfile {
                        column,
                        descending: index.columns()[0].descending(),
                        maximum: metadata.storage_length(),
                        selection: binary::BinaryIndexSelection::All,
                    });
                    super::delete::edit::<_, { binary::MAX_ENTRY_BYTES }>(
                        image,
                        &mut reader,
                        index,
                        schema.definition,
                        references,
                        &profile,
                        &mut patches,
                    )?;
                    continue;
                }
            }
            if kind == ScalarType::Decimal {
                if !matches!(
                    image.catalog().header().format(),
                    FileFormat::Ace2016 | FileFormat::Ace2019
                ) {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                let profile = DecimalProfile {
                    column,
                    descending: index.columns()[0].descending(),
                    kind: DecimalType::from_column(&schema.columns[column])?,
                    selection: DecimalIndexSelection::All,
                };
                edit::<_, { decimal::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    index,
                    schema.definition,
                    change,
                    &profile,
                    &mut patches,
                )?;
            } else if kind == ScalarType::Guid {
                if !matches!(
                    image.catalog().header().format(),
                    FileFormat::Ace2016 | FileFormat::Ace2019
                ) {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                let profile = GuidProfile {
                    column,
                    selection: super::super::guid::GuidIndexSelection::All,
                    descending: index.columns()[0].descending(),
                };
                edit::<_, { guid::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    index,
                    schema.definition,
                    change,
                    &profile,
                    &mut patches,
                )?;
            } else {
                let profile = SingleProfile {
                    column,
                    kind: NumericKey::new(kind)?,
                    descending: index.columns()[0].descending(),
                };
                #[cfg(feature = "ace16-write-preview")]
                if super::split::supported_scalar(kind)
                    && let Some(growth) = splitting.as_deref_mut()
                    && let Change::Update(updates) = change
                {
                    super::split::edit(
                        image,
                        &mut reader,
                        index,
                        updates,
                        &profile,
                        &mut patches.indexes,
                        growth,
                    )?;
                    continue;
                }
                edit::<_, { encoding::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    index,
                    schema.definition,
                    change,
                    &profile,
                    &mut patches,
                )?;
            }
        } else {
            if !matches!(
                image.catalog().header().format(),
                FileFormat::Ace2016 | FileFormat::Ace2019
            ) {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            reader.work(2 * index.columns().len() * schema.columns.len())?;
            let has_guid = index.columns().iter().any(|key| {
                schema.columns.iter().any(|column| {
                    column.number() == key.number()
                        && column.scalar_type() == Some(ScalarType::Guid)
                })
            });
            if composite::has_decimal(index, schema)? {
                let profile = match change {
                    Change::Update(_) => TupleProfile::<DecimalValue, DecimalColumns>::new_decimal(
                        index,
                        schema,
                        reader.budget,
                        reader.cancelled,
                    ),
                    #[cfg(feature = "ace16-write-preview")]
                    Change::Delete(_) | Change::DeleteUpdate(_, _) => {
                        TupleProfile::<DecimalValue, DecimalColumns>::new_decimal_for_delete(
                            index,
                            schema,
                            reader.budget,
                            reader.cancelled,
                        )
                    }
                }?;
                edit::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    index,
                    schema.definition,
                    change,
                    &profile,
                    &mut patches,
                )?;
            } else if has_guid {
                let profile = match change {
                    Change::Update(_) => TupleProfile::<MixedValue>::new(
                        index,
                        schema,
                        reader.budget,
                        reader.cancelled,
                    ),
                    #[cfg(feature = "ace16-write-preview")]
                    Change::Delete(_) | Change::DeleteUpdate(_, _) => {
                        TupleProfile::<MixedValue>::new_for_delete(
                            index,
                            schema,
                            reader.budget,
                            reader.cancelled,
                        )
                    }
                }?;
                edit::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    index,
                    schema.definition,
                    change,
                    &profile,
                    &mut patches,
                )?;
            } else {
                let profile = match change {
                    Change::Update(_) => {
                        TupleProfile::<i64>::new(index, schema, reader.budget, reader.cancelled)
                    }
                    #[cfg(feature = "ace16-write-preview")]
                    Change::Delete(_) | Change::DeleteUpdate(_, _) => {
                        TupleProfile::<i64>::new_for_delete(
                            index,
                            schema,
                            reader.budget,
                            reader.cancelled,
                        )
                    }
                }?;
                edit::<_, { composite::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    index,
                    schema.definition,
                    change,
                    &profile,
                    &mut patches,
                )?;
            }
        }
    }
    check(budget, cancelled)?;
    Ok(patches)
}

fn edit<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    change: Change<'_>,
    profile: &P,
    patches: &mut PatchSet,
) -> Result<(), EditError> {
    #[cfg(not(feature = "ace16-write-preview"))]
    let Change::Update(updates) = change;
    #[cfg(feature = "ace16-write-preview")]
    let updates = match change {
        Change::Update(updates) => updates,
        Change::Delete(references) => {
            return super::delete::edit::<P, N>(
                image, reader, index, owner, references, profile, patches,
            );
        }
        Change::DeleteUpdate(references, updates) => {
            return super::combined::edit::<P, N>(
                image,
                reader,
                index,
                owner,
                (references, updates),
                profile,
                patches,
            );
        }
    };
    let keys::Prepared {
        tree,
        entries,
        mode,
    } = keys::prepare::<_, N>(image, reader, index, owner, updates, profile)?;
    let mut remaining = entries.as_slice();
    tree.rewrite::<_, N>(
        image,
        &mut remaining,
        &Rewrite { profile, mode },
        &mut patches.indexes,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() {
        return Err(EditError::Preservation);
    }
    Ok(())
}
