use crate::profile::{
    MAX_BIND_BYTES, MAX_BIND_DEPTH, MAX_BIND_NODES, MAX_PARAMETERS, SOURCE_SCHEMA_POLICY,
};
use crate::{
    QueryError, Source,
    plan::{ReadPlan, Slot},
};
use tablecraft_schema::{SchemaName, TypeDescriptor};
use tablecraft_sql::query::{Query, Statement};

mod calls;
mod editing;
mod expression;
pub(crate) use editing::{BoundUpdate, bind_update};
mod named;
mod parameters;
mod projection;
mod relation;
mod retrieval;
mod source_schema;

/// Prepared retrieval with resolved slots. A host may load bounded metadata or
/// snapshots while supplying columns; binding invokes no row-scan callback.
/// Plans intentionally omit Debug/Serde to avoid leaking query/identifier payloads.
pub struct PreparedQuery {
    pub(crate) read: ReadPlan,
    pub(crate) parameters: Vec<Parameter>,
}
pub(crate) struct Parameter {
    pub name: SchemaName,
    pub data_type: TypeDescriptor,
}
pub(super) struct Binder<'a> {
    source: &'a dyn Source,
    parameters: Vec<Parameter>,
    parameter_scope: Vec<usize>,
    // Named sources and scalar calls remain unavailable to UPDATE binding.
    retrieval: bool,
    active_queries: Vec<String>,
    nodes: usize,
    bytes: usize,
}
impl Binder<'_> {
    fn tick(&mut self, depth: usize) -> Result<(), QueryError> {
        self.nodes += 1;
        if depth > MAX_BIND_DEPTH || self.nodes > MAX_BIND_NODES {
            return Err(QueryError::StructureLimit);
        }
        self.charge(std::mem::size_of::<crate::plan::Expr>())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), QueryError> {
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
    let _span = tracing::debug_span!("query.bind").entered();
    let Statement::Read(read) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    if query.parameters.len() > MAX_PARAMETERS {
        return Err(QueryError::StructureLimit);
    }
    let mut binder = Binder {
        source,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        active_queries: Vec::new(),
        nodes: 0,
        bytes: 0,
    };
    binder.parameters = binder.bind_parameters(&query.parameters)?;
    binder.parameter_scope = binder.scope_indices(binder.parameters.len())?;
    let mut read = binder.read(read, &[], 1)?;
    projection::prune(&mut read, &mut binder)?;
    Ok(PreparedQuery {
        read,
        parameters: binder.parameters,
    })
}
fn resolve(
    field: &tablecraft_sql::select::FieldReference,
    slots: &[Slot],
) -> Result<Option<usize>, QueryError> {
    let mut found = None;
    let mut index = 0;
    for slot in slots {
        if SOURCE_SCHEMA_POLICY
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
