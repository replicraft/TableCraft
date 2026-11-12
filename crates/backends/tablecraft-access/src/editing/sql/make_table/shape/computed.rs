//! Fixed computed outputs use established query carriers; native fields keep native sizes.
mod expressions;
mod scope;
use super::{NativeMakeTableError as Error, charge_name, check, native_column};
use expressions::Clause;
use scope::Scope;
use tablecraft_ace::{CreateColumn, InspectError, TableSchema};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::MakeTablePlan;
use tablecraft_schema::{NameComparison, ScalarType};
use tablecraft_sql::{
    query::{Parameter, ReadBody, Statement},
    select::{Name, Projection, Select, Table},
};

/// Validate relation/field syntax before any user-record scan. Computed output
/// carrier checks follow complete query staging; NULL/Dynamic never infer a type.
pub(in crate::editing::sql::make_table) fn preflight(
    select: &Select,
    schema: &TableSchema,
    parameters: &[Parameter],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    let scope = Scope::new(select, schema, parameters)?;
    let mut direct_only = true;
    for projection in &select.projection {
        check(work, cancelled)?;
        match projection {
            Projection::Field { field, alias } => {
                if let Some(source) = scope.field(field, false, work, cancelled)? {
                    let name = alias.as_ref().unwrap_or(&field.name).as_str();
                    let _native_declaration = native_column(name, source)?;
                } else {
                    direct_only = false;
                }
            }
            Projection::Wildcard(qualifier) => {
                qualify_wildcard(qualifier.as_ref(), scope.table, work, cancelled)?;
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
                expressions::qualify(expression, &scope, Clause::Projection, work, cancelled)?;
                direct_only = false;
            }
            _ => return Err(Error::Shape),
        }
    }
    for expression in &select.group_by {
        expressions::qualify(expression, &scope, Clause::Group, work, cancelled)?;
    }
    if let Some(expression) = &select.having {
        expressions::qualify(expression, &scope, Clause::Having, work, cancelled)?;
    }
    for key in &select.order_by {
        expressions::qualify(&key.expression, &scope, Clause::Order, work, cancelled)?;
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
    let scope = Scope::new(select, schema, &staged.retrieval().parameters)?;

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
            qualify_wildcard(qualifier.as_ref(), scope.table, work, cancelled)?;
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
                let source = scope.field(field, false, work, cancelled)?;
                let name = alias.as_ref().unwrap_or(&field.name).as_str();
                let column = match source {
                    Some(source) => native_column(name, source)?,
                    None => {
                        fixed_column(declaration.name(), declaration.data_type().scalar_type())?
                    }
                };
                if column.scalar_type() != declaration.data_type().scalar_type() {
                    return Err(Error::Shape);
                }
                column
            }
            Projection::Expression {
                alias: Some(alias), ..
            } => fixed_column(alias.as_str(), declaration.data_type().scalar_type())?,
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

fn fixed_column(name: &str, scalar: ScalarType) -> Result<CreateColumn<'_>, Error> {
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
        // Query Text lengths do not establish native UTF16 capacity; an
        // unconstrained Decimal does not establish native precision/scale.
        return Err(Error::Shape);
    }

    Ok(CreateColumn::new(name, scalar))
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
