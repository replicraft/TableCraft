//! One bounded retained projection consumed by the existing query aggregate plan.
use crate::{Error, controls::check};
use tablecraft_access::NativeCollationResolver;
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{
    QueryError, Source,
    source::{CollationResolver, TextCollationId},
};
use tablecraft_schema::Column;

pub(super) const SOURCE_NAME: &str = "TotalsSource";

pub(super) struct Snapshot {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Value>>,
    pub policies: Vec<Option<TextCollationId>>,
    pub orders: Vec<Option<NativeSortOrder>>,
}

impl Source for Snapshot {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if table != SOURCE_NAME {
            return Err(QueryError::UnknownName);
        }
        Ok(&self.columns)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.columns(table)?;
        Ok(&self.policies)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.columns(table)?;
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}

pub(super) struct Policies<'a, 'b> {
    pub source: &'a Snapshot,
    pub resolver: &'a NativeCollationResolver<'b>,
}

impl CollationResolver for Policies<'_, '_> {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        let position = usize::try_from(policy.host_key()).map_err(|_| QueryError::Source)?;
        let order = self
            .source
            .orders
            .get(position)
            .and_then(|order| *order)
            .ok_or(QueryError::Source)?;
        self.resolver.resolve_native(order)
    }
}

pub(super) fn copy_value(
    value: &Value,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, Error> {
    check(cancelled)?;
    work.charge(Resource::Steps, 1)?;
    match value {
        Value::Text(text) => {
            work.charge(Resource::Steps, text.len() as u64)?;
            work.charge_allocation(text.len() as u64, 1)?;
            let mut owned = String::new();
            owned.try_reserve_exact(text.len())?;
            owned.push_str(text);
            Ok(Value::Text(owned))
        }
        Value::Binary(bytes) => {
            work.charge(Resource::Steps, bytes.len() as u64)?;
            work.charge_allocation(bytes.len() as u64, 1)?;
            let mut owned = Vec::new();
            owned.try_reserve_exact(bytes.len())?;
            owned.extend_from_slice(bytes);
            Ok(Value::Binary(owned))
        }
        // NO_CLONE: The Text/Binary branches perform separately charged fallible buffer copies
        // or refusal. This remaining match arm copies scalar carriers from a borrowed
        // enum with no underlying heap payload.
        _ => Ok(value.clone()),
    }
}
