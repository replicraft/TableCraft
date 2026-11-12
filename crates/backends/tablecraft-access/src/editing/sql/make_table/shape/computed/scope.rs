//! Metadata-only symbol classification mirrors the query owner's field precedence.
use super::Error;
use crate::editing::sql::make_table::shape::{charge_name, resolve_optional, table};
use tablecraft_ace::{NativeColumn, TableSchema};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::NameComparison;
use tablecraft_sql::{
    query::Parameter,
    select::{FieldReference, Projection, Select, Table},
};

pub(super) struct Scope<'a> {
    pub(super) table: &'a Table,
    pub(super) schema: &'a TableSchema,
    parameters: &'a [Parameter],
    projections: &'a [Projection],
}

impl<'a> Scope<'a> {
    pub(super) fn new(
        select: &'a Select,
        schema: &'a TableSchema,
        parameters: &'a [Parameter],
    ) -> Result<Self, Error> {
        Ok(Self {
            table: table(select)?,
            schema,
            parameters,
            projections: &select.projection,
        })
    }

    pub(super) fn field(
        &self,
        reference: &FieldReference,
        aliases: bool,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<&NativeColumn>, Error> {
        if let Some(column) = resolve_optional(reference, self.table, self.schema, work, cancelled)?
        {
            return Ok(Some(column));
        }
        if reference.qualifier.is_some() {
            return Err(Error::Shape);
        }

        for parameter in self.parameters {
            charge_name(
                parameter.name.as_str(),
                reference.name.as_str(),
                work,
                cancelled,
            )?;
            if NameComparison::AsciiInsensitive
                .equals(parameter.name.as_str(), reference.name.as_str())
            {
                return Ok(None);
            }
        }
        if aliases {
            for projection in self.projections {
                let alias = match projection {
                    Projection::Field { alias, .. } | Projection::Expression { alias, .. } => alias,
                    Projection::Wildcard(_) => continue,
                };
                let Some(alias) = alias else {
                    continue;
                };
                charge_name(alias.as_str(), reference.name.as_str(), work, cancelled)?;
                if NameComparison::AsciiInsensitive.equals(alias.as_str(), reference.name.as_str())
                {
                    return Ok(None);
                }
            }
        }

        Err(Error::Shape)
    }
}
