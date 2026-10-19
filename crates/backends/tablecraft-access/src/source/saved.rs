//! Explicit saved SELECT reads sharing the native source's work controls.
use super::NativeSource;
use crate::{SavedSelect, SavedSelectError, reconstruct_saved_select};
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, read_saved_query_definition};
use tablecraft_sql::ParseLimits;
use thiserror::Error;

/// Native metadata or owned-syntax failure; no names, SQL or row payloads.
#[derive(Debug, Error)]
pub enum NativeSavedSelectError {
    /// Native catalog/query-fragment reading failed.
    #[error(transparent)]
    Native(#[from] InspectError),
    /// Native fragments could not be reconstructed under the supported profile.
    #[error(transparent)]
    Reconstruction(#[from] SavedSelectError),
}

impl<R: Read + Seek> NativeSource<'_, R> {
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
    ) -> Result<SavedSelect, NativeSavedSelectError> {
        let definition = read_saved_query_definition(
            self.source.get_mut(),
            self.scope,
            name,
            self.budget.get_mut(),
            self.cancelled,
        )?;
        Ok(reconstruct_saved_select(
            definition,
            limits,
            self.budget.get_mut(),
            self.cancelled,
        )?)
    }
}
