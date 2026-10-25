//! Resolve captured host bindings before rows and retain per-operation revisions.
use super::Runtime;
use crate::{
    QueryError, Source,
    external_sources::{ExternalBinding, MAX_EXTERNAL_SOURCES, SourceNamespace},
    source::SourceKey,
};
use tablecraft_core::limits::Resource;

pub(super) struct RuntimeSource<'a> {
    pub key: SourceKey,
    pub source: &'a dyn Source,
}

impl<'a, 'source> Runtime<'a, 'source> {
    pub(super) fn register_external_sources(
        &mut self,
        bindings: &[ExternalBinding],
    ) -> Result<(), QueryError> {
        for binding in bindings {
            self.tick()?;
            self.budget
                .charge(Resource::Steps, binding.comparison_work(binding.request())?)?;
            let root = self.source;
            let resolved = root
                .external_source(binding.request())?
                .ok_or(QueryError::SchemaChanged)?;
            self.tick()?;
            if resolved.key() != binding.key {
                return Err(QueryError::SchemaChanged);
            }
            self.remember_external_binding(binding)?;
            let mut known = false;
            for index in 0..self.external_sources.len() {
                self.tick()?;
                let previous = &self.external_sources[index];
                if previous.key == binding.key {
                    if !std::ptr::addr_eq(previous.source, resolved.source()) {
                        return Err(QueryError::SchemaChanged);
                    }
                    known = true;
                    break;
                }
            }
            if !known {
                if self.external_sources.len() == MAX_EXTERNAL_SOURCES {
                    return Err(QueryError::StructureLimit);
                }
                self.budget
                    .charge_allocation(1, size_of::<RuntimeSource<'_>>() as u64)?;
                self.external_sources
                    .try_reserve_exact(1)
                    .map_err(|_| QueryError::Allocation)?;
                self.external_sources.push(RuntimeSource {
                    key: binding.key,
                    source: resolved.source(),
                });
            }
        }
        Ok(())
    }

    fn remember_external_binding(&mut self, binding: &ExternalBinding) -> Result<(), QueryError> {
        let discovery = self.discovery;
        {
            let remembered = discovery
                .external_sources
                .try_borrow()
                .map_err(|_| QueryError::Source)?;
            for previous in remembered.iter() {
                self.tick()?;
                self.budget.charge(
                    Resource::Steps,
                    previous.comparison_work(binding.request())?,
                )?;
                if previous.matches(binding.request()) {
                    return if previous.key == binding.key {
                        Ok(())
                    } else {
                        Err(QueryError::SchemaChanged)
                    };
                }
                if previous.key == binding.key {
                    return Err(QueryError::SchemaChanged);
                }
            }
            if remembered.len() == MAX_EXTERNAL_SOURCES {
                return Err(QueryError::StructureLimit);
            }
        }
        self.budget
            .charge_allocation(1, size_of::<ExternalBinding>() as u64)?;
        let database = self.external_string(&binding.database)?;
        let connection = binding
            .connection
            .as_deref()
            .map(|value| self.external_string(value))
            .transpose()?;
        let mut remembered = discovery
            .external_sources
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?;
        remembered
            .try_reserve_exact(1)
            .map_err(|_| QueryError::Allocation)?;
        remembered.push(ExternalBinding {
            database,
            connection,
            connection_bracketed: binding.connection_bracketed,
            key: binding.key,
        });
        Ok(())
    }

    fn external_string(&mut self, value: &str) -> Result<String, QueryError> {
        self.tick()?;
        self.budget.charge(Resource::Bytes, value.len() as u64)?;
        let mut copied = String::new();
        copied
            .try_reserve_exact(value.len())
            .map_err(|_| QueryError::Allocation)?;
        copied.push_str(value);
        Ok(copied)
    }

    pub(in crate::execution) fn namespace_source(
        &mut self,
        namespace: SourceNamespace,
    ) -> Result<&'source dyn Source, QueryError> {
        self.tick()?;
        if namespace == SourceNamespace::Root {
            return Ok(self.source);
        }
        for index in 0..self.external_sources.len() {
            self.tick()?;
            let candidate = &self.external_sources[index];
            if namespace == SourceNamespace::External(candidate.key) {
                return Ok(candidate.source);
            }
        }
        Err(QueryError::SchemaChanged)
    }
}
