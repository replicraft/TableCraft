//! Explicit saved SELECT/UNION/crosstab reads sharing the native source's work controls.
use super::{NativeSource, query_error};
use crate::{
    SavedRead, SavedReadError, SavedSelect, reconstruct_saved_read, reconstruct_saved_select,
};
use std::cell::OnceCell;
use tablecraft_ace::{CatalogInspection, InspectError, read_saved_query_definition};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;
use tablecraft_query::QueryError;
use tablecraft_sql::ParseLimits;
use tablecraft_sql::query::Query;
use thiserror::Error;

/// Native metadata or owned-syntax failure; no names, SQL or row payloads.
#[derive(Debug, Error)]
pub enum NativeSavedReadError {
    /// Native catalog/query-fragment reading failed.
    #[error(transparent)]
    Native(#[from] InspectError),
    /// Native fragments could not be reconstructed under the supported profile.
    #[error(transparent)]
    Reconstruction(#[from] SavedReadError),
}

/// Compatibility name for errors from the SELECT-only read method.
pub type NativeSavedSelectError = NativeSavedReadError;

impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    /// Explicitly read one named saved SELECT/UNION/crosstab as inert owned syntax.
    /// Metadata, reconstruction and later reads share this source's budget and
    /// cancellation. Sources must stay stable/offline; execution remains separate.
    pub fn read_saved_query(
        &mut self,
        name: &str,
        limits: ParseLimits,
    ) -> Result<SavedRead, NativeSavedReadError> {
        if self.budget.shared() && (self.cancelled)() {
            return Err(InspectError::Cancelled.into());
        }
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?;
        let definition = read_saved_query_definition(
            self.source.get_mut(),
            self.scope,
            name,
            &mut budget,
            self.cancelled,
        )?;
        Ok(reconstruct_saved_read(
            definition,
            limits,
            &mut budget,
            self.cancelled,
        )?)
    }
    /// Explicitly read one named saved local SELECT as owned syntax. Metadata,
    /// reconstruction and later table reads share this source's cumulative budget
    /// and cancellation callback. The source must stay stable/offline.
    ///
    /// Reading does not execute the query, resolve saved-query source names,
    /// grant owner-access authority or activate functions/providers. Hosts must
    /// separately prepare/execute the returned query through the query library.
    pub fn read_saved_select(
        &mut self,
        name: &str,
        limits: ParseLimits,
    ) -> Result<SavedSelect, NativeSavedReadError> {
        if self.budget.shared() && (self.cancelled)() {
            return Err(InspectError::Cancelled.into());
        }
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?;
        let definition = read_saved_query_definition(
            self.source.get_mut(),
            self.scope,
            name,
            &mut budget,
            self.cancelled,
        )?;
        Ok(reconstruct_saved_select(
            definition,
            limits,
            &mut budget,
            self.cancelled,
        )?)
    }
}

pub(super) struct QueryEntry {
    name: String,
    native_id: i32,
    definition: OnceCell<SavedRead>,
}

impl QueryEntry {
    pub(super) fn has_name(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}
pub(super) fn entries(
    catalog: &CatalogInspection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<QueryEntry>, InspectError> {
    let count = catalog
        .catalog()
        .objects()
        .iter()
        .filter(|object| object.kind() == ObjectKind::Query)
        .count();
    budget.charge_allocation(count as u64, size_of::<QueryEntry>() as u64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for (object, origin) in catalog.catalog().objects().iter().zip(catalog.origins()) {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        if object.kind() != ObjectKind::Query {
            continue;
        }
        let name = object.name().as_str();
        budget.charge_allocation(name.len() as u64, 1)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(name.len())
            .map_err(|_| InspectError::Allocation)?;
        owned.push_str(name);
        result.push(QueryEntry {
            name: owned,
            native_id: origin.native_id(),
            definition: OnceCell::new(),
        });
    }
    Ok(result)
}
impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    pub(super) fn named_query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        if (self.cancelled)() {
            return Err(QueryError::Cancelled);
        }
        self.budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?
            .charge(
                Resource::Steps,
                self.queries.len() as u64 + self.entries.len() as u64 + 1,
            )?;
        let mut matches = self
            .queries
            .iter()
            .filter(|entry| entry.name.eq_ignore_ascii_case(name));
        let Some(entry) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some()
            || self
                .entries
                .iter()
                .any(|table| table.name.eq_ignore_ascii_case(name))
        {
            return Err(QueryError::AmbiguousName);
        }
        if entry.definition.get().is_none() {
            let mut source = self
                .source
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let definition = read_saved_query_definition(
                &mut *source,
                self.scope,
                &entry.name,
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?;
            if definition.origin().native_id() != entry.native_id {
                return Err(QueryError::SchemaChanged);
            }
            let saved = reconstruct_saved_read(
                definition,
                ParseLimits::default(),
                &mut budget,
                self.cancelled,
            )
            .map_err(reconstruction_error)?;
            entry
                .definition
                .set(saved)
                .map_err(|_| QueryError::Source)?;
        }
        Ok(Some(
            entry.definition.get().ok_or(QueryError::Source)?.query(),
        ))
    }
}
fn reconstruction_error(error: SavedReadError) -> QueryError {
    match error {
        SavedReadError::Unsupported => QueryError::Unsupported,
        SavedReadError::Cancelled => QueryError::Cancelled,
        SavedReadError::Allocation => QueryError::Allocation,
        SavedReadError::Limit(error) => QueryError::Limit(error),
        SavedReadError::SyntaxLimit => QueryError::StructureLimit,
        SavedReadError::InvalidMetadata | SavedReadError::Parse(_) => QueryError::Source,
    }
}
