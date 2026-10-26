//! Runtime domain preparation uses the selected Source and shared operation budget.
mod finish;
mod input;
mod scalar;
mod text;
use super::{Binder, projection};
use crate::{
    BindingDepth, QueryError, Source,
    discovery::Materialized,
    plan::{DomainFunction, ReadPlan},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::dates::parsing::ReferenceYear;

pub(crate) struct DomainContext<'a> {
    pub inherited_read_depth: usize,
    pub binding_depth: BindingDepth,
    pub owner: crate::external_sources::SourceNamespace,
    pub external_sources: &'a [crate::external_sources::ExternalBinding],
    pub source: &'a dyn Source,
    pub materialized: &'a [Materialized<'a>],
    pub budget: &'a mut ResourceBudget,
    pub discover: bool,
    pub reference_year: Option<ReferenceYear>,
    pub declared_arithmetic: bool,
    pub cancelled: Option<&'a dyn Fn() -> bool>,
}

// Inner Err requests explicit execution staging; the outer Err is an actual
// binding failure. None retains the existing missing-criteria-field result.
pub(crate) struct BoundDomain {
    pub read: ReadPlan,
    pub external_sources: Vec<crate::external_sources::ExternalBinding>,
}

type DomainPreparation = Result<Option<BoundDomain>, crate::named_identity::SchemaRequest>;

#[inline(never)]
pub(crate) fn bind_domain(
    context: DomainContext<'_>,
    kind: DomainFunction,
    expression: &str,
    domain: &str,
    criteria: Option<&str>,
) -> Result<DomainPreparation, QueryError> {
    let mut input = input::prepare_input(context, kind, expression, domain, criteria)?;
    let relation = input.binder.relation(
        input.syntax.from.first().ok_or(QueryError::Unsupported)?,
        &[],
        1,
        input.owner,
    );
    finish::finish(
        input.binder,
        relation,
        input.owner,
        kind,
        input.value.as_ref(),
        input.filter.as_ref(),
    )
}
