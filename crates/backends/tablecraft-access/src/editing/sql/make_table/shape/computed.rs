//! Fixed computed outputs use established query carriers; native fields keep native sizes.
mod expressions;
use super::{NativeMakeTableError as Error, charge_name, check, native_column, resolve, table};
use tablecraft_ace::{CreateColumn, InspectError, TableSchema};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::MakeTablePlan;
use tablecraft_schema::{NameComparison, ScalarType};
use tablecraft_sql::{
    query::{ReadBody, Statement},
    select::{Expression, Name, Projection, Select, Table},
};

/// Validate relation/field syntax before any user-record scan. Computed output
/// carrier checks follow complete query staging; NULL/Dynamic never infer a type.
pub(in crate::editing::sql::make_table) fn preflight(
    select: &Select,
    schema: &TableSchema,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    let table = table(select)?;
    let mut direct_only = true;
    for projection in &select.projection {
        check(work, cancelled)?;
        match projection {
            Projection::Field { field, alias } => {
                let source = resolve(field, table, schema, work, cancelled)?;
                let name = alias.as_ref().unwrap_or(&field.name).as_str();
                let _native_declaration = native_column(name, source)?;
            }
            Projection::Wildcard(qualifier) => {
                qualify_wildcard(qualifier.as_ref(), table, work, cancelled)?;
                for source in schema.columns() {
                    check(work, cancelled)?;
                    let _native_declaration = native_column(source.name(), source)?;
                }
                // The query owner expands declarations without changing the AST.
                direct_only = false;
            }
            Projection::Expression {
                expression,
                alias: Some(_),
                ..
            } => {
                expressions::qualify(expression, table, schema, work, cancelled)?;
                direct_only = false;
            }
            _ => return Err(Error::Shape),
        }
    }
    for key in &select.order_by {
        let Expression::Field(reference) = &key.expression else {
            return Err(Error::Shape);
        };
        resolve(reference, table, schema, work, cancelled)?;
    }
    Ok(direct_only)
}

pub(in crate::editing::sql::make_table) fn staged_columns<'a>(
    staged: &'a MakeTablePlan,
    schema: &TableSchema,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CreateColumn<'a>>, Error> {
    let Statement::Read(read) = &staged.retrieval().statement else {
        return Err(Error::Shape);
    };
    let ReadBody::Select(select) = &read.body else {
        return Err(Error::Shape);
    };
    let table = table(select)?;

    work.charge_allocation(
        staged.columns().len() as u64,
        size_of::<CreateColumn<'_>>() as u64,
    )
    .map_err(InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(staged.columns().len())
        .map_err(|_| InspectError::Allocation)?;

    let mut declarations = staged.columns().iter();
    for projection in &select.projection {
        check(work, cancelled)?;
        if let Projection::Wildcard(qualifier) = projection {
            qualify_wildcard(qualifier.as_ref(), table, work, cancelled)?;
            for source in schema.columns() {
                check(work, cancelled)?;
                let declaration = declarations.next().ok_or(Error::Shape)?;
                charge_name(declaration.name(), source.name(), work, cancelled)?;
                // The same native source supplied the query's metadata. Preserve
                // its declaration order; a different expansion cannot borrow a
                // neighboring field's physical Text or Decimal declaration.
                if declaration.name() != source.name() {
                    return Err(Error::Shape);
                }
                let column = native_column(declaration.name(), source)?;
                if column.scalar_type() != declaration.data_type().scalar_type() {
                    return Err(Error::Shape);
                }
                columns.push(column);
            }
            continue;
        }

        let declaration = declarations.next().ok_or(Error::Shape)?;
        let column = match projection {
            Projection::Field { field, alias } => {
                let source = resolve(field, table, schema, work, cancelled)?;
                let name = alias.as_ref().unwrap_or(&field.name).as_str();
                let column = native_column(name, source)?;
                if column.scalar_type() != declaration.data_type().scalar_type() {
                    return Err(Error::Shape);
                }
                column
            }
            Projection::Expression {
                alias: Some(alias), ..
            } => {
                let scalar = declaration.data_type().scalar_type();
                if !matches!(
                    scalar,
                    ScalarType::Boolean
                        | ScalarType::Byte
                        | ScalarType::Int16
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Single
                        | ScalarType::Double
                        | ScalarType::Currency
                        | ScalarType::DateTime
                        | ScalarType::Guid
                ) {
                    // Query Unicode-scalar lengths do not establish native UTF16
                    // capacity; unconstrained Decimal has no native p/s declaration.
                    return Err(Error::Shape);
                }
                CreateColumn::new(alias.as_str(), scalar)
            }
            _ => return Err(Error::Shape),
        };
        // Query nullability is never a Required policy. No source PK, counter,
        // default, AZL or description is transferred to this manual column.
        columns.push(column);
    }

    check(work, cancelled)?;
    if declarations.next().is_some() {
        return Err(Error::Shape);
    }

    Ok(columns)
}

fn qualify_wildcard(
    qualifier: Option<&Name>,
    table: &Table,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(work, cancelled)?;
    if let Some(qualifier) = qualifier {
        let actual = table.alias.as_ref().unwrap_or(&table.name);
        charge_name(qualifier.as_str(), actual.as_str(), work, cancelled)?;
        if !NameComparison::AsciiInsensitive.equals(qualifier.as_str(), actual.as_str()) {
            return Err(Error::Shape);
        }
    }

    Ok(())
}
