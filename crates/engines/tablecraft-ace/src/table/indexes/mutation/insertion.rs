//! Typed request membership, followed by physical-owner continuation and wire facts.
pub(in crate::table::indexes) mod membership;
mod preparation;
mod profiles;
#[cfg(test)]
mod tests;

use super::split::{
    AddedPage,
    allocation::{Promotion, Rebase},
};
use super::{IndexDataPatch, NumericKey, composite::TupleProfile, profile::SingleProfile};
use crate::table::indexes::{composite::MixedValue, guid::GuidProfile};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    header::check,
    table::{
        indexes::{constraints::PRIMARY_INDEX, key::KeyRow},
        mutation::append::State,
    },
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
    pub added: Vec<AddedPage>,
    pub promotions: Vec<Promotion>,
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
enum PreparedKind {
    Numeric(Prepared<SingleProfile>),
    Guid(Prepared<GuidProfile>),
    Tuple(Prepared<TupleProfile>),
    Mixed(Prepared<TupleProfile<MixedValue>>),
}

pub(in crate::table) struct PreparedIndex {
    kind: PreparedKind,
}

impl PreparedIndex {
    pub(in crate::table) fn source_pages(&self) -> &[u32] {
        match &self.kind {
            PreparedKind::Numeric(plan) => &plan.result.source_pages,
            PreparedKind::Guid(plan) => &plan.result.source_pages,
            PreparedKind::Tuple(plan) => &plan.result.source_pages,
            PreparedKind::Mixed(plan) => &plan.result.source_pages,
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
        use crate::table::indexes::{composite, guid};
        match self.kind {
            PreparedKind::Numeric(plan) => {
                plan.finish::<ENTRY_BUFFER>(image, checked, append, budget, cancelled)
            }
            PreparedKind::Guid(plan) => {
                plan.finish::<{ guid::MAX_ENTRY_BYTES }>(image, checked, append, budget, cancelled)
            }
            PreparedKind::Tuple(plan) => plan.finish::<{ composite::MAX_ENTRY_BYTES }>(
                image, checked, append, budget, cancelled,
            ),
            PreparedKind::Mixed(plan) => plan.finish::<{ composite::MAX_GUID_ENTRY_BYTES }>(
                image, checked, append, budget, cancelled,
            ),
        }
    }
}

pub(in crate::table) fn prepare(
    image: &NativeImage,
    checked: &TableCheck,
    physical: u32,
    requests: &[InsertRow<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedIndex, EditError> {
    use crate::table::indexes::{composite, guid};
    check(budget, cancelled)?;
    let index = qualify(checked, physical, budget, cancelled)?;
    match profiles::bind(checked, index, budget, cancelled)? {
        profiles::Profile::Numeric(profile) => preparation::prepare::<_, ENTRY_BUFFER>(
            image, checked, index, profile, requests, budget, cancelled,
        )
        .map(|plan| PreparedIndex {
            kind: PreparedKind::Numeric(plan),
        }),
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

pub(in crate::table::indexes) fn qualify<'a>(
    checked: &'a TableCheck,
    physical: u32,
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
                || field.and_then(|column| column.scalar_type()) != Some(ScalarType::Int32)
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
