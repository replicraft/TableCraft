//! Typed request membership, followed by physical-owner continuation and wire facts.
pub(in crate::table::indexes::mutation) mod binding;
pub(in crate::table::indexes) mod membership;
mod preparation;
#[cfg(all(test, feature = "journal"))]
mod primary_tests;
#[cfg(feature = "ace16-write-preview")]
mod profiles;
#[cfg(feature = "journal")]
pub(in crate::table) mod root_split;
#[cfg(all(test, feature = "ace16-write-preview"))]
mod tests;

#[cfg(feature = "ace16-write-preview")]
use super::composite::TupleProfile;
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
use super::retained::Preserved;
#[cfg(feature = "ace16-write-preview")]
use super::split::{
    AddedPage,
    allocation::{Promotion, Rebase},
};
use super::{IndexDataPatch, NumericKey, profile::SingleProfile};
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
use crate::table::indexes::binary::BinaryProfile;
#[cfg(feature = "journal")]
use crate::table::indexes::binary::{self, BinaryIndexSelection};
#[cfg(feature = "journal")]
use crate::table::indexes::text;
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
use crate::table::indexes::text::TextProfile;
#[cfg(feature = "ace16-write-preview")]
use crate::table::indexes::{
    composite::{DecimalColumns, DecimalValue, MixedValue},
    decimal::DecimalProfile,
    encoded_tuple::EncodedTupleProfile,
    guid::GuidProfile,
};
#[cfg(feature = "ace16-write-preview")]
use crate::table::mutation::append::State;
use crate::{
    EditError, IndexCheckStatus, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    header::check,
    table::indexes::{constraints::PRIMARY_INDEX, key::KeyRow},
};
use preparation::Prepared;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

const ENTRY_BUFFER: usize = size_of::<u8>() + size_of::<i64>() + 2 * size_of::<u32>();

pub(in crate::table) struct InsertRow<'a> {
    pub reference: u32,
    pub values: &'a [Value],
}

impl KeyRow for InsertRow<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        self.values.get(column)
    }
}

struct ExpectedKey {
    bytes: Vec<u8>,
    reference: u32,
}

/// Completed builder facts. Independent replay derives its own request keys.
pub(in crate::table) struct IndexInsertion {
    pub patches: Vec<IndexDataPatch>,
    #[cfg(feature = "ace16-write-preview")]
    pub added: Vec<AddedPage>,
    #[cfg(feature = "ace16-write-preview")]
    pub promotions: Vec<Promotion>,
    #[cfg(feature = "ace16-write-preview")]
    pub rebases: Vec<Rebase>,
    physical: u32,
    root: u32,
    usage: u32,
    original_count: usize,
    source_pages: Vec<u32>,
    distinct_added: usize,
    expected: Vec<ExpectedKey>,
}

impl IndexInsertion {
    pub(in crate::table) fn source_pages(&self) -> &[u32] {
        &self.source_pages
    }
    pub(in crate::table) fn physical(&self) -> u32 {
        self.physical
    }
    pub(in crate::table) fn root(&self) -> u32 {
        self.root
    }
    pub(in crate::table) fn usage(&self) -> u32 {
        self.usage
    }
    pub(in crate::table) fn distinct_added(&self) -> usize {
        self.distinct_added
    }
    pub(in crate::table) fn original_count(&self) -> usize {
        self.original_count
    }
    pub(in crate::table) fn expected(&self) -> impl ExactSizeIterator<Item = (&[u8], u32)> + '_ {
        self.expected
            .iter()
            .map(|entry| (entry.bytes.as_slice(), entry.reference))
    }
}

/// Fixed profiles and typed keys stay owned until this phase is consumed.
#[cfg(feature = "ace16-write-preview")]
enum PreparedKind<'a> {
    Numeric(Prepared<SingleProfile>),
    Text(Prepared<Preserved<TextProfile>>),
    Binary(Prepared<Preserved<BinaryProfile<'static>>>),
    Encoded(Prepared<Preserved<EncodedTupleProfile<'a>>>),
    Decimal(Prepared<DecimalProfile>),
    Guid(Prepared<GuidProfile>),
    Tuple(Prepared<TupleProfile>),
    Mixed(Prepared<TupleProfile<MixedValue>>),
    DecimalTuple(Prepared<TupleProfile<DecimalValue, DecimalColumns>>),
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) struct PreparedIndex<'a> {
    kind: PreparedKind<'a>,
}

#[cfg(feature = "ace16-write-preview")]
impl PreparedIndex<'_> {
    pub(in crate::table) fn source_pages(&self) -> &[u32] {
        match &self.kind {
            PreparedKind::Numeric(plan) => &plan.result.source_pages,
            PreparedKind::Text(plan) => &plan.result.source_pages,
            PreparedKind::Binary(plan) => &plan.result.source_pages,
            PreparedKind::Encoded(plan) => &plan.result.source_pages,
            PreparedKind::Decimal(plan) => &plan.result.source_pages,
            PreparedKind::Guid(plan) => &plan.result.source_pages,
            PreparedKind::Tuple(plan) => &plan.result.source_pages,
            PreparedKind::Mixed(plan) => &plan.result.source_pages,
            PreparedKind::DecimalTuple(plan) => &plan.result.source_pages,
        }
    }

    pub(in crate::table) fn continue_with_append(
        self,
        image: &NativeImage,
        checked: &TableCheck,
        append: State,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(IndexInsertion, State), EditError> {
        use crate::table::indexes::{binary, composite, decimal, encoded_tuple, guid, text};
        match self.kind {
            PreparedKind::Numeric(plan) => {
                plan.finish::<ENTRY_BUFFER>(image, checked, append, budget, cancelled)
            }
            PreparedKind::Decimal(plan) => plan
                .finish::<{ decimal::MAX_ENTRY_BYTES }>(image, checked, append, budget, cancelled),
            PreparedKind::Text(plan) => {
                plan.finish::<{ text::MAX_ENTRY_BYTES }>(image, checked, append, budget, cancelled)
            }
            PreparedKind::Binary(plan) => plan
                .finish::<{ binary::MAX_ENTRY_BYTES }>(image, checked, append, budget, cancelled),
            PreparedKind::Encoded(plan) => plan.finish::<{ encoded_tuple::MAX_ENTRY_BYTES }>(
                image, checked, append, budget, cancelled,
            ),
            PreparedKind::Guid(plan) => {
                plan.finish::<{ guid::MAX_ENTRY_BYTES }>(image, checked, append, budget, cancelled)
            }
            PreparedKind::Tuple(plan) => plan.finish::<{ composite::MAX_ENTRY_BYTES }>(
                image, checked, append, budget, cancelled,
            ),
            PreparedKind::Mixed(plan) => plan.finish::<{ composite::MAX_GUID_ENTRY_BYTES }>(
                image, checked, append, budget, cancelled,
            ),
            PreparedKind::DecimalTuple(plan) => plan.finish::<{ composite::MAX_GUID_ENTRY_BYTES }>(
                image, checked, append, budget, cancelled,
            ),
        }
    }
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) fn prepare<'a>(
    image: &NativeImage,
    checked: &'a TableCheck,
    physical: u32,
    requests: &[InsertRow<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedIndex<'a>, EditError> {
    use crate::table::indexes::{binary, composite, decimal, encoded_tuple, guid, text};
    check(budget, cancelled)?;
    let index = qualify(checked, physical, budget, cancelled)?;
    match profiles::bind(checked, index, budget, cancelled)? {
        profiles::Profile::Numeric(profile) => preparation::prepare::<_, ENTRY_BUFFER>(
            image, checked, index, profile, requests, budget, cancelled,
        )
        .map(|plan| PreparedIndex {
            kind: PreparedKind::Numeric(plan),
        }),
        profiles::Profile::Decimal(profile) => {
            preparation::prepare::<_, { decimal::MAX_ENTRY_BYTES }>(
                image, checked, index, profile, requests, budget, cancelled,
            )
            .map(|plan| PreparedIndex {
                kind: PreparedKind::Decimal(plan),
            })
        }
        profiles::Profile::Text(profile) => preparation::prepare::<_, { text::MAX_ENTRY_BYTES }>(
            image, checked, index, profile, requests, budget, cancelled,
        )
        .map(|plan| PreparedIndex {
            kind: PreparedKind::Text(plan),
        }),
        profiles::Profile::Binary(profile) => {
            preparation::prepare::<_, { binary::MAX_ENTRY_BYTES }>(
                image, checked, index, profile, requests, budget, cancelled,
            )
            .map(|plan| PreparedIndex {
                kind: PreparedKind::Binary(plan),
            })
        }
        profiles::Profile::Encoded(profile) => {
            preparation::prepare::<_, { encoded_tuple::MAX_ENTRY_BYTES }>(
                image, checked, index, profile, requests, budget, cancelled,
            )
            .map(|plan| PreparedIndex {
                kind: PreparedKind::Encoded(plan),
            })
        }
        profiles::Profile::Guid(profile) => preparation::prepare::<_, { guid::MAX_ENTRY_BYTES }>(
            image, checked, index, profile, requests, budget, cancelled,
        )
        .map(|plan| PreparedIndex {
            kind: PreparedKind::Guid(plan),
        }),
        profiles::Profile::Tuple(profile) => {
            preparation::prepare::<_, { composite::MAX_ENTRY_BYTES }>(
                image, checked, index, profile, requests, budget, cancelled,
            )
            .map(|plan| PreparedIndex {
                kind: PreparedKind::Tuple(plan),
            })
        }
        profiles::Profile::DecimalTuple(profile) => {
            preparation::prepare::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
                image, checked, index, profile, requests, budget, cancelled,
            )
            .map(|plan| PreparedIndex {
                kind: PreparedKind::DecimalTuple(plan),
            })
        }
        profiles::Profile::Mixed(profile) => {
            preparation::prepare::<_, { composite::MAX_GUID_ENTRY_BYTES }>(
                image, checked, index, profile, requests, budget, cancelled,
            )
            .map(|plan| PreparedIndex {
                kind: PreparedKind::Mixed(plan),
            })
        }
    }
}

#[derive(Clone, Copy)]
enum PrimaryProfile {
    LegacyInt32,
    #[cfg(feature = "journal")]
    NormalIntegral,
}

impl PrimaryProfile {
    fn permits(self, kind: Option<ScalarType>) -> bool {
        match self {
            Self::LegacyInt32 => kind == Some(ScalarType::Int32),
            #[cfg(feature = "journal")]
            Self::NormalIntegral => matches!(
                kind,
                Some(ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency)
            ),
        }
    }
}

pub(in crate::table) fn qualify<'a>(
    checked: &'a TableCheck,
    physical: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    qualify_primary(
        checked,
        physical,
        PrimaryProfile::LegacyInt32,
        budget,
        cancelled,
    )
}

#[cfg(feature = "journal")]
pub(in crate::table) fn qualify_actions<'a>(
    checked: &'a TableCheck,
    physical: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    qualify_primary(
        checked,
        physical,
        PrimaryProfile::NormalIntegral,
        budget,
        cancelled,
    )
}

fn qualify_primary<'a>(
    checked: &'a TableCheck,
    physical: u32,
    profile: PrimaryProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    if physical as usize >= checked.schema().layout.physical_indexes {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let first = checked
        .indexes()
        .iter()
        .find(|entry| entry.index().data_number() == physical)
        .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?
        .index();
    let mut primary = false;
    for entry in checked
        .indexes()
        .iter()
        .filter(|entry| entry.index().data_number() == physical)
    {
        check(budget, cancelled)?;
        let index = entry.index();
        let constraints = index
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        if entry.status() != IndexCheckStatus::Checked
            || index.native_type() > PRIMARY_INDEX
            || index.root_page() != first.root_page()
            || index.usage_map() != first.usage_map()
            || index.flags() != first.flags()
            || index.columns() != first.columns()
            || index.columns().is_empty()
            || (constraints.ignore_nulls() && (constraints.required() || constraints.primary_key()))
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if constraints.primary_key() {
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            let mut field = None;
            for column in checked.schema().columns() {
                check(budget, cancelled)?;
                if column.number() == key.number() {
                    field = Some(column);
                }
            }
            if primary
                || !profile.permits(field.and_then(|column| column.scalar_type()))
                || key.descending()
                || !constraints.unique()
                || !constraints.required()
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            primary = true;
        }
    }
    Ok(first)
}

/// Checked fixed-key preparation with no allocation continuation capability.
/// The caller owns source/event admission and proves the complete final image.
#[cfg(feature = "journal")]
pub(in crate::table) fn prepare_existing(
    image: &NativeImage,
    checked: &TableCheck,
    physical: u32,
    requests: &[InsertRow<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexInsertion, EditError> {
    check(budget, cancelled)?;
    let index = qualify_actions(checked, physical, budget, cancelled)?;
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let mut field = None;
    for (position, column) in checked.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        if column.number() == key.number() {
            field = Some((position, column));
        }
    }
    let (position, column) = field.ok_or(EditError::Preservation)?;
    if column.scalar_type() == Some(ScalarType::Text) {
        let profile = Preserved(TextProfile::new(position, column, key.descending())?);
        let prepared = preparation::prepare::<_, { text::MAX_ENTRY_BYTES }>(
            image, checked, index, profile, requests, budget, cancelled,
        )?;
        return prepared.finish_fitting::<{ text::MAX_ENTRY_BYTES }>(budget, cancelled);
    }
    if column.scalar_type() == Some(ScalarType::Binary) {
        let profile = Preserved(BinaryProfile {
            column: position,
            descending: key.descending(),
            maximum: column.storage_length(),
            selection: BinaryIndexSelection::All,
        });
        let prepared = preparation::prepare::<_, { binary::MAX_ENTRY_BYTES }>(
            image, checked, index, profile, requests, budget, cancelled,
        )?;
        return prepared.finish_fitting::<{ binary::MAX_ENTRY_BYTES }>(budget, cancelled);
    }
    let kind = match column.scalar_type() {
        Some(kind @ (ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency)) => kind,
        _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
    };
    let profile = SingleProfile {
        column: position,
        kind: NumericKey::new(kind)?,
        descending: key.descending(),
    };
    let prepared: Prepared<SingleProfile> = preparation::prepare::<_, ENTRY_BUFFER>(
        image, checked, index, profile, requests, budget, cancelled,
    )?;
    prepared.finish_existing::<ENTRY_BUFFER>(image, checked, budget, cancelled)
}
