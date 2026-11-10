//! Source-free binder fixture shared by bounded planning owner tests.
use super::Binder;
use crate::{QueryError, Source};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::Column;

pub(in crate::binding) struct NoSource;
impl Source for NoSource {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        panic!("constant metadata must not request source columns")
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("constant metadata must not scan sources")
    }
}

pub(in crate::binding) fn binder<'a>(
    budget: Option<&'a mut ResourceBudget>,
    cancelled: Option<&'a dyn Fn() -> bool>,
) -> Binder<'a> {
    Binder {
        inherited_read_depth: 0,
        pending_named_depth: 0,
        next_relation_occurrence: 0,
        binding_depth: Default::default(),
        aliases: Default::default(),
        budget,
        cancelled,
        source: &NoSource,
        external_sources: Vec::new(),
        external_providers: Vec::new(),
        materialized: &[],
        discover: false,
        reference_year: None,
        declared_arithmetic: false,
        pending_schema: None,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        mutation_scalar: false,
        collection_criteria: false,
        active_queries: Vec::new(),
        owner: crate::external_sources::SourceNamespace::Root,
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    }
}
