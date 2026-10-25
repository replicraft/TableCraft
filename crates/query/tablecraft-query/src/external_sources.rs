//! Captured external identities, separate from relation and named-query names.
use crate::{
    QueryError,
    source::{ExternalSourceRequest, SourceKey},
};

pub(crate) const MAX_EXTERNAL_SOURCES: usize = 256;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SourceNamespace {
    #[default]
    Root,
    External(SourceKey),
}

pub(crate) struct ExternalBinding {
    pub database: String,
    pub connection: Option<String>,
    pub connection_bracketed: bool,
    pub key: SourceKey,
}

impl ExternalBinding {
    pub fn request(&self) -> ExternalSourceRequest<'_> {
        ExternalSourceRequest::new(
            &self.database,
            self.connection.as_deref(),
            self.connection_bracketed,
        )
    }

    pub fn matches(&self, request: ExternalSourceRequest<'_>) -> bool {
        self.database == request.database()
            && self.connection.as_deref() == request.connection()
            && self.connection_bracketed == request.connection_bracketed()
    }

    pub fn comparison_work(&self, request: ExternalSourceRequest<'_>) -> Result<u64, QueryError> {
        let length = self
            .database
            .len()
            .checked_add(self.connection.as_ref().map_or(0, String::len))
            .and_then(|length| length.checked_add(request.database().len()))
            .and_then(|length| length.checked_add(request.connection().map_or(0, str::len)))
            .and_then(|length| length.checked_add(1))
            .ok_or(QueryError::StructureLimit)?;
        u64::try_from(length).map_err(|_| QueryError::StructureLimit)
    }
}
