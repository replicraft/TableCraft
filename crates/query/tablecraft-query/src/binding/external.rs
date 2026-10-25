//! Metadata-only external FROM scope selection over explicit host bindings.
use super::Binder;
use crate::{
    QueryError, Source,
    external_sources::{ExternalBinding, MAX_EXTERNAL_SOURCES, SourceNamespace},
    source::{ExternalSourceRequest, SourceKey},
};
use tablecraft_core::limits::Resource;
use tablecraft_sql::select::ExternalDatabase;

impl<'a> Binder<'a> {
    pub(super) fn external_namespace(
        &mut self,
        external: Option<&ExternalDatabase>,
        depth: usize,
    ) -> Result<SourceNamespace, QueryError> {
        let Some(external) = external else {
            return Ok(self.owner);
        };
        let request = ExternalSourceRequest::new(
            &external.database,
            external.connection.as_deref(),
            external.connection_bracketed,
        );
        self.capture_external_source(request, None, depth)
    }

    fn capture_external_source(
        &mut self,
        request: ExternalSourceRequest<'_>,
        expected: Option<SourceKey>,
        depth: usize,
    ) -> Result<SourceNamespace, QueryError> {
        for index in 0..self.external_sources.len() {
            self.tick(depth)?;
            let work = self.external_sources[index].comparison_work(request)?;
            if let Some(budget) = &mut self.budget {
                budget.charge(Resource::Steps, work)?;
            }
            if self.external_sources[index].matches(request) {
                let key = self.external_sources[index].key;
                if expected.is_some_and(|expected| expected != key) {
                    return Err(QueryError::SchemaChanged);
                }
                return Ok(SourceNamespace::External(key));
            }
        }
        if self.external_sources.len() == MAX_EXTERNAL_SOURCES {
            return Err(QueryError::StructureLimit);
        }
        self.tick(depth)?;
        if let Some(budget) = &mut self.budget {
            let work = request
                .database()
                .len()
                .checked_add(request.connection().map_or(0, str::len))
                .ok_or(QueryError::StructureLimit)?;
            budget.charge(Resource::Steps, work as u64)?;
        }
        let resolved = self
            .source
            .external_source(request)?
            .ok_or(if expected.is_some() {
                QueryError::SchemaChanged
            } else {
                QueryError::Unsupported
            })?;
        self.tick(depth)?;
        if expected.is_some_and(|expected| expected != resolved.key()) {
            return Err(QueryError::SchemaChanged);
        }
        for index in 0..self.external_sources.len() {
            self.tick(depth)?;
            if self.external_sources[index].key == resolved.key() {
                // One key identifies one immutable binding, not two selector mappings.
                return Err(QueryError::Source);
            }
        }
        let database = self.copy_name(request.database())?;
        let connection = request
            .connection()
            .map(|value| self.copy_name(value))
            .transpose()?;
        self.charge(size_of::<ExternalBinding>() + size_of::<&dyn Source>())?;
        self.external_sources
            .try_reserve_exact(1)
            .map_err(|_| QueryError::Allocation)?;
        self.external_providers
            .try_reserve_exact(1)
            .map_err(|_| QueryError::Allocation)?;
        let key = resolved.key();
        self.external_sources.push(ExternalBinding {
            database,
            connection,
            connection_bracketed: request.connection_bracketed(),
            key,
        });
        self.external_providers.push(resolved.source());
        Ok(SourceNamespace::External(key))
    }

    pub(super) fn seed_external_sources(
        &mut self,
        bindings: &[ExternalBinding],
    ) -> Result<(), QueryError> {
        for binding in bindings {
            let namespace =
                self.capture_external_source(binding.request(), Some(binding.key), 1)?;
            if namespace != SourceNamespace::External(binding.key) {
                return Err(QueryError::SchemaChanged);
            }
        }
        Ok(())
    }

    pub(super) fn with_owner<T>(
        &mut self,
        owner: SourceNamespace,
        bind: impl FnOnce(&mut Self) -> Result<T, QueryError>,
    ) -> Result<T, QueryError> {
        let previous = std::mem::replace(&mut self.owner, owner);
        let result = bind(self);
        self.owner = previous;
        result
    }

    pub(super) fn namespace_source(
        &mut self,
        namespace: SourceNamespace,
        depth: usize,
    ) -> Result<&'a dyn Source, QueryError> {
        if namespace == SourceNamespace::Root {
            return Ok(self.source);
        }
        for index in 0..self.external_sources.len() {
            self.tick(depth)?;
            if namespace == SourceNamespace::External(self.external_sources[index].key) {
                return self
                    .external_providers
                    .get(index)
                    .copied()
                    .ok_or(QueryError::Source);
            }
        }
        Err(QueryError::Source)
    }
}
