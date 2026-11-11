//! Shared closed mutation binder setup; callers supply their exact syntax gate.
use super::Binder;
use crate::Source;
use tablecraft_core::limits::ResourceBudget;

impl<'a> Binder<'a> {
    pub(super) fn mutation(
        source: &'a dyn Source,
        budget: &'a mut ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            inherited_read_depth: 0,
            pending_named_depth: 0,
            next_relation_occurrence: 0,
            binding_depth: Default::default(),
            aliases: Default::default(),
            budget: Some(budget),
            cancelled: Some(cancelled),
            source,
            external_sources: Vec::new(),
            external_providers: Vec::new(),
            materialized: &[],
            discover: false,
            reference_year: None,
            declared_arithmetic: false,
            pending_schema: None,
            parameters: Vec::new(),
            parameter_scope: Vec::new(),
            retrieval: false,
            mutation_scalar: true,
            implicit_parameter_start: None,
            collection_criteria: false,
            active_queries: Vec::new(),
            owner: crate::external_sources::SourceNamespace::Root,
            nodes: 0,
            policy_visits: 0,
            bytes: 0,
        }
    }
}
