use crate::profile::{
    MAX_BIND_BYTES, MAX_BIND_DEPTH, MAX_BIND_NODES, MAX_PARAMETERS, SOURCE_SCHEMA_POLICY,
};
use crate::{
    QueryError, Source,
    plan::{PreparedBody, Slot},
};
use tablecraft_schema::{SchemaName, TypeDescriptor};
use tablecraft_sql::query::{Query, Statement};

mod arrays;
mod calls;
mod cashflows;
mod crosstab;
mod distinctrow;
mod domain;
pub(crate) use domain::bind_domain;
mod editing;
mod expression;
mod grouping;
pub(crate) use editing::{BoundUpdate, bind_update};
mod multivalue;
mod named;
mod parameters;
mod projection;
mod relation;
mod retrieval;
mod source_schema;
mod text_policy;

/// Prepared retrieval with resolved slots. A host may load bounded metadata or
/// snapshots while supplying columns; binding invokes no row-scan callback.
/// Plans intentionally omit Debug/Serde to avoid leaking query/identifier payloads.
pub struct PreparedQuery {
    pub(crate) body: PreparedBody,
    pub(crate) parameters: Vec<Parameter>,
}
pub(crate) struct Parameter {
    pub name: SchemaName,
    pub data_type: TypeDescriptor,
}
pub(super) struct Binder<'a> {
    budget: Option<&'a mut tablecraft_core::limits::ResourceBudget>,
    cancelled: Option<&'a dyn Fn() -> bool>,
    source: &'a dyn Source,
    materialized: &'a [crate::discovery::Materialized<'a>],
    discover: bool,
    pending_schema: Option<String>,
    parameters: Vec<Parameter>,
    parameter_scope: Vec<usize>,
    // Named sources and scalar calls remain unavailable to UPDATE binding.
    retrieval: bool,
    active_queries: Vec<String>,
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
    match prepare_stage(query, source, &[], None, None, false)? {
        Ok(plan) => Ok(plan),
        Err(_) => Err(QueryError::Unsupported),
    }
}

pub(crate) type Preparation = Result<PreparedQuery, String>;

pub(crate) fn prepare_stage<'a>(
    query: &Query,
    source: &'a dyn Source,
    materialized: &'a [crate::discovery::Materialized<'a>],
    budget: Option<&'a mut tablecraft_core::limits::ResourceBudget>,
    cancelled: Option<&'a dyn Fn() -> bool>,
    discover: bool,
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
        materialized,
        discover,
        pending_schema: None,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        active_queries: Vec::new(),
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    };
    let result = (|| {
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
    match result {
        Ok(body) => Ok(Ok(PreparedQuery {
            body,
            parameters: binder.parameters,
        })),
        Err(QueryError::Unsupported) if binder.pending_schema.is_some() => Ok(Err(binder
            .pending_schema
            .take()
            .ok_or(QueryError::Unsupported)?)),
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
