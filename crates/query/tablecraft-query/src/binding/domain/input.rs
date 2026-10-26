//! Parse and configure domain binding before recursive source expansion.
use super::{Binder, DomainContext, text};
use crate::plan::DomainFunction;
use crate::{QueryError, external_sources::SourceNamespace};
use tablecraft_sql::select::{Expression, Select};

pub(super) struct DomainInput<'a> {
    pub binder: Binder<'a>,
    pub syntax: Select,
    pub value: Option<Expression>,
    pub filter: Option<Expression>,
    pub owner: SourceNamespace,
}

#[inline(never)]
pub(super) fn prepare_input<'a>(
    context: DomainContext<'a>,
    kind: DomainFunction,
    expression: &str,
    domain: &str,
    criteria: Option<&str>,
) -> Result<DomainInput<'a>, QueryError> {
    let DomainContext {
        inherited_read_depth,
        binding_depth,
        owner,
        external_sources,
        source,
        materialized,
        budget,
        cancelled,
        discover,
        reference_year,
        declared_arithmetic,
    } = context;
    let (syntax, value, filter) = {
        let mut text = text::Text {
            budget,
            cancelled,
            binding_depth,
        };
        let syntax = text.source(domain)?;
        let value = if expression == "*" && kind == DomainFunction::Count {
            None
        } else {
            Some(text.expression(expression)?)
        };
        let filter = criteria
            .filter(|s| !s.trim().is_empty())
            .map(|s| text.expression(s))
            .transpose()?;
        (syntax, value, filter)
    };
    let mut binder = Binder {
        inherited_read_depth,
        pending_named_depth: 0,
        next_relation_occurrence: 0,
        binding_depth,
        aliases: Default::default(),
        source,
        external_sources: Vec::new(),
        external_providers: Vec::new(),
        materialized,
        discover,
        reference_year,
        declared_arithmetic,
        pending_schema: None,
        budget: Some(budget),
        cancelled,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        collection_criteria: false,
        active_queries: Vec::new(),
        owner,
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    };
    binder.seed_external_sources(external_sources)?;
    Ok(DomainInput {
        binder,
        syntax,
        value,
        filter,
        owner,
    })
}
