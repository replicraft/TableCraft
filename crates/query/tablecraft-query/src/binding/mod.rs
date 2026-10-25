use crate::profile::{
    MAX_BIND_BYTES, MAX_BIND_DEPTH, MAX_BIND_NODES, MAX_PARAMETERS, SOURCE_SCHEMA_POLICY,
};
use crate::{
    OutputType, QueryError, Source,
    plan::{PreparedBody, Slot},
};
use tablecraft_expressions::functions::dates::parsing::ReferenceYear;
use tablecraft_sql::query::{Query, Statement};

mod arrays;
mod calls;
mod cashflows;
mod crosstab;
mod declarations;
mod distinctrow;
mod domain;
pub(crate) use domain::{DomainContext, bind_domain};
mod editing;
mod expression;
mod expression_identity;
mod external;
mod grouping;
pub(crate) use editing::{BoundUpdate, bind_update};
mod literals;
mod multivalue;
mod named;
mod parameters;
pub(crate) use parameters::Parameter;
mod projection;
mod relation;
mod result_types;
mod retrieval;
mod source_schema;
mod text_policy;

/// Caller-issued lexical settings for metadata-only retrieval preparation.
/// No clock, locale discovery or row scan supplies these settings.
#[derive(Clone, Copy, Default)]
pub struct PreparationOptions {
    /// Capture the explicit engineering decimal integer-literal typing policy.
    /// Staged execution pairs this with declaration-sensitive arithmetic;
    /// already prepared literal carriers cannot be changed during execution.
    pub declared_arithmetic: bool,
    /// Explicit lexical default for SQL date tokens that omit their year.
    /// Resolved dates are captured in the plan; execution cannot replace them.
    pub reference_year: Option<ReferenceYear>,
}

/// Prepared retrieval with resolved slots and captured lexical constants.
/// Binding invokes no row-scan callback. Plans omit Debug/Serde to avoid leaking
/// query and identifier payloads.
pub struct PreparedQuery {
    pub(crate) body: PreparedBody,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) external_sources: Vec<crate::external_sources::ExternalBinding>,
}

pub(super) struct Binder<'a> {
    budget: Option<&'a mut tablecraft_core::limits::ResourceBudget>,
    cancelled: Option<&'a dyn Fn() -> bool>,
    source: &'a dyn Source,
    external_sources: Vec<crate::external_sources::ExternalBinding>,
    external_providers: Vec<&'a dyn Source>,
    materialized: &'a [crate::discovery::Materialized<'a>],
    discover: bool,
    reference_year: Option<ReferenceYear>,
    declared_arithmetic: bool,
    pending_schema: Option<crate::named_identity::NamedIdentity>,
    parameters: Vec<Parameter>,
    parameter_scope: Vec<usize>,
    // Named sources and scalar calls remain unavailable to UPDATE binding.
    retrieval: bool,
    collection_criteria: bool,
    active_queries: Vec<crate::named_identity::NamedIdentity>,
    owner: crate::external_sources::SourceNamespace,
    nodes: usize,
    policy_visits: usize,
    bytes: usize,
}

impl Binder<'_> {
    fn tick(&mut self, depth: usize) -> Result<(), QueryError> {
        if self.cancelled.is_some_and(|cancel| cancel()) {
            return Err(QueryError::Cancelled);
        }
        if let Some(budget) = &mut self.budget {
            budget.charge(tablecraft_core::limits::Resource::Steps, 1)?;
        }
        self.nodes += 1;
        if depth > MAX_BIND_DEPTH || self.nodes > MAX_BIND_NODES {
            return Err(QueryError::StructureLimit);
        }
        self.charge(std::mem::size_of::<crate::plan::Expr>())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), QueryError> {
        if let Some(budget) = &mut self.budget {
            budget.charge(tablecraft_core::limits::Resource::Bytes, bytes as u64)?;
        }
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(QueryError::StructureLimit)?;
        if self.bytes > MAX_BIND_BYTES {
            return Err(QueryError::StructureLimit);
        }
        Ok(())
    }
}
/// Bind a retrieval query, resolving names and rejecting unsupported forms before
/// scanning. ASCII-insensitive identifiers are the initial explicit profile.
pub fn prepare(query: &Query, source: &dyn Source) -> Result<PreparedQuery, QueryError> {
    prepare_with_options(query, source, PreparationOptions::default())
}

/// Bind without scanning, capturing explicitly supplied date-token defaults.
/// Executing the prepared plan with another reference year affects runtime Text
/// conversions only; its already resolved SQL date tokens remain unchanged.
pub fn prepare_with_options(
    query: &Query,
    source: &dyn Source,
    options: PreparationOptions,
) -> Result<PreparedQuery, QueryError> {
    let stage = StageOptions {
        preparation: options,
        ..Default::default()
    };
    match prepare_stage(query, source, &[], None, None, stage)? {
        Ok(plan) => Ok(plan),
        Err(_) => Err(QueryError::Unsupported),
    }
}

pub(crate) type Preparation = Result<PreparedQuery, crate::named_identity::SchemaRequest>;

#[derive(Clone, Copy, Default)]
pub(crate) struct StageOptions<'a> {
    pub owner: crate::external_sources::SourceNamespace,
    pub external_sources: &'a [crate::external_sources::ExternalBinding],
    pub preparation: PreparationOptions,
    pub discover: bool,
    pub parameterless: bool,
}

pub(crate) fn prepare_stage<'a>(
    query: &Query,
    source: &'a dyn Source,
    materialized: &'a [crate::discovery::Materialized<'a>],
    budget: Option<&'a mut tablecraft_core::limits::ResourceBudget>,
    cancelled: Option<&'a dyn Fn() -> bool>,
    options: StageOptions,
) -> Result<Preparation, QueryError> {
    let _span = tracing::debug_span!("query.bind").entered();
    if !matches!(query.statement, Statement::Read(_) | Statement::Crosstab(_)) {
        return Err(QueryError::Unsupported);
    }
    if query.parameters.len() > MAX_PARAMETERS {
        return Err(QueryError::StructureLimit);
    }
    let mut binder = Binder {
        budget,
        cancelled,
        source,
        external_sources: Vec::new(),
        external_providers: Vec::new(),
        materialized,
        discover: options.discover,
        reference_year: options.preparation.reference_year,
        declared_arithmetic: options.preparation.declared_arithmetic,
        pending_schema: None,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        collection_criteria: false,
        active_queries: Vec::new(),
        owner: options.owner,
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    };
    let result = (|| {
        binder.seed_external_sources(options.external_sources)?;
        binder.parameters = binder.bind_parameters(&query.parameters)?;
        binder.parameter_scope = binder.scope_indices(binder.parameters.len())?;
        let body = match &query.statement {
            Statement::Read(read) => {
                let mut read = binder.read(read, &[], 1)?;
                projection::prune(&mut read, &mut binder)?;
                PreparedBody::Read(read)
            }
            Statement::Crosstab(cross) => PreparedBody::Crosstab(binder.crosstab(cross)?),
            _ => return Err(QueryError::Unsupported),
        };
        Ok(body)
    })();
    if options.parameterless && !binder.parameters.is_empty() {
        return Err(QueryError::Parameter);
    }
    match result {
        Ok(body) => Ok(Ok(PreparedQuery {
            body,
            parameters: binder.parameters,
            external_sources: binder.external_sources,
        })),
        Err(QueryError::Unsupported) if binder.pending_schema.is_some() => {
            Ok(Err(crate::named_identity::SchemaRequest {
                identity: binder
                    .pending_schema
                    .take()
                    .ok_or(QueryError::Unsupported)?,
                external_sources: binder.external_sources,
            }))
        }
        Err(error) => Err(error),
    }
}
fn resolve(
    field: &tablecraft_sql::select::FieldReference,
    slots: &[Slot],
) -> Result<Option<usize>, QueryError> {
    let mut found = None;
    let mut index = 0;
    for slot in slots {
        if slot.member.is_none()
            && SOURCE_SCHEMA_POLICY
                .names
                .equals(&slot.name, field.name.as_str())
            && field.qualifier.as_ref().is_none_or(|q| {
                slot.qualifier
                    .as_deref()
                    .is_some_and(|s| SOURCE_SCHEMA_POLICY.names.equals(s, q.as_str()))
            })
        {
            if found.is_some() {
                return Err(QueryError::AmbiguousName);
            }
            found = Some((index, slot.available));
        }
        index += usize::from(slot.available);
    }
    match found {
        Some((_, false)) => Err(QueryError::Unsupported),
        Some((index, true)) => Ok(Some(index)),
        None => Ok(None),
    }
}

impl PreparedQuery {
    /// Captured output names for an ordinary read plan, without executing it.
    /// Top-level crosstabs return None because headings can require execution.
    /// Names preserve aliases and possible duplicates, as ordinary results do.
    pub fn output_names(&self) -> Option<&[String]> {
        match &self.body {
            PreparedBody::Read(read) => Some(&read.names),
            PreparedBody::Crosstab(_) => None,
        }
    }

    /// Conservative output carrier/constraint/nullability metadata, established
    /// without scanning records. Dynamic crosstabs have no static output width.
    /// Describes both materialized and declared runtime arithmetic: differing
    /// successful carriers remain Dynamic. Completed results instead describe
    /// their selected execution profile. No source records establish these types.
    /// Dynamic entries do not assert a carrier or storage representation.
    pub fn output_types(&self) -> Option<&[OutputType]> {
        match &self.body {
            PreparedBody::Read(read) => Some(&read.output_types),
            PreparedBody::Crosstab(cross) => cross.output_types.as_deref(),
        }
    }

    /// Static output text-policy provenance, without reading records or choosing
    /// comparers. Dynamic crosstabs return None because their width is unknown.
    /// Fixed schemas return one entry per output, including default provenance for
    /// non-Text outputs. This is not full result-type inference or native precedence.
    pub fn output_text_policies(&self) -> Option<&[crate::source::TextPolicy]> {
        match &self.body {
            PreparedBody::Read(read) => Some(&read.text_policies),
            PreparedBody::Crosstab(cross) => cross.text_policies.as_deref(),
        }
    }
}
