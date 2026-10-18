mod expression;
mod relation;
mod retrieval;
use crate::{
    QueryError, Source,
    plan::{ReadPlan, Slot},
};
use tablecraft_schema::ScalarType;
use tablecraft_sql::query::{Query, Statement};

/// Prepared retrieval with resolved slots; no source reads occur during binding.
/// Plans intentionally omit Debug/Serde to avoid leaking query/identifier payloads.
pub struct PreparedQuery {
    pub(crate) read: ReadPlan,
    pub(crate) parameters: Vec<Parameter>,
}
pub(crate) struct Parameter {
    pub name: String,
    pub kind: ScalarType,
    pub max_length: Option<usize>,
}
pub(super) struct Binder<'a> {
    source: &'a dyn Source,
    parameters: Vec<Parameter>,
    nodes: usize,
    bytes: usize,
}
impl Binder<'_> {
    fn tick(&mut self, depth: usize) -> Result<(), QueryError> {
        self.nodes += 1;
        if depth > 16 || self.nodes > 2048 {
            return Err(QueryError::StructureLimit);
        }
        self.charge(std::mem::size_of::<crate::plan::Expr>())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), QueryError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(QueryError::StructureLimit)?;
        if self.bytes > 1_048_576 {
            return Err(QueryError::StructureLimit);
        }
        Ok(())
    }
}
/// Bind a retrieval query, resolving names and rejecting unsupported forms before
/// scanning. ASCII-insensitive identifiers are the initial explicit profile.
pub fn prepare(query: &Query, source: &dyn Source) -> Result<PreparedQuery, QueryError> {
    let Statement::Read(read) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    if query.parameters.len() > 256 {
        return Err(QueryError::StructureLimit);
    }
    let mut parameters: Vec<Parameter> = Vec::new();
    for parameter in &query.parameters {
        if parameter.name.as_str().len() > 255 {
            return Err(QueryError::StructureLimit);
        }
        if parameters
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(parameter.name.as_str()))
        {
            return Err(QueryError::AmbiguousName);
        }
        let kind = match parameter.data_type.name.as_str() {
            "BIT" | "YESNO" | "BOOLEAN" | "LOGICAL" => ScalarType::Boolean,
            "BYTE" | "TINYINT" => ScalarType::Byte,
            "SHORT" | "SMALLINT" => ScalarType::Int16,
            "LONG" | "LONGINTEGER" | "INTEGER" | "INT" => ScalarType::Int32,
            "BIGINT" | "LONGLONG" | "LARGE NUMBER" => ScalarType::Int64,
            "SINGLE" | "REAL" | "IEEESINGLE" => ScalarType::Single,
            "DOUBLE" | "FLOAT" | "IEEEDOUBLE" => ScalarType::Double,
            "CURRENCY" | "MONEY" => ScalarType::Currency,
            "DECIMAL" | "NUMERIC" => ScalarType::Decimal,
            "TEXT" | "CHAR" | "CHARACTER" | "VARCHAR" | "LONGTEXT" | "MEMO" | "LONGCHAR" => {
                ScalarType::Text
            }
            "GUID" | "UNIQUEIDENTIFIER" => ScalarType::Guid,
            "DATETIME" | "DATE" | "TIME" | "TIMESTAMP" => ScalarType::DateTime,
            "DATETIMEEXTENDED" => ScalarType::DateTimeExtended,
            "BINARY" | "IMAGE" | "LONGBINARY" => ScalarType::Binary,
            _ => return Err(QueryError::Unsupported),
        };
        let max_length = match parameter.data_type.arguments.as_slice() {
            [] => None,
            [n] if kind == ScalarType::Text && *n > 0 => Some(*n as usize),
            _ => return Err(QueryError::Unsupported),
        };
        parameters.push(Parameter {
            name: parameter.name.as_str().into(),
            kind,
            max_length,
        });
    }
    let mut binder = Binder {
        source,
        parameters,
        nodes: 0,
        bytes: 0,
    };
    let read = binder.read(read, &[], 1)?;
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
    for (i, slot) in slots.iter().enumerate() {
        if slot.name.eq_ignore_ascii_case(field.name.as_str())
            && field.qualifier.as_ref().is_none_or(|q| {
                slot.qualifier
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case(q.as_str()))
            })
        {
            if found.is_some() {
                return Err(QueryError::AmbiguousName);
            }
            found = Some(i);
        }
    }
    Ok(found)
}
