//! Closed scalar predicate retrieval shared by UPDATE and DELETE.
pub(super) mod syntax;
use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ReadNode, ReadPlan, RelationNode},
};

pub(super) fn admit(expression: &Expr, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    admit_with(expression, binder, true)
}

fn admit_with(expression: &Expr, binder: &mut Binder<'_>, like: bool) -> Result<(), QueryError> {
    binder.work()?;
    match expression {
        Expr::Parameter(_)
        | Expr::Literal(_)
        | Expr::NumericLiteral(_)
        | Expr::DateLiteral(..)
        | Expr::Field { .. } => Ok(()),
        Expr::Unary(_, value)
        | Expr::IsNull(value, _)
        | Expr::Convert(_, value)
        | Expr::DateCall(_, value) => admit_with(value, binder, like),
        Expr::Like(left, right, _) if like => {
            admit_with(left, binder, like)?;
            admit_with(right, binder, like)
        }
        Expr::Binary(_, left, right) => {
            admit_with(left, binder, like)?;
            admit_with(right, binder, like)
        }
        Expr::Between(value, first, last, _)
        | Expr::IIf(value, first, last)
        | Expr::DateAdd(value, first, last) => {
            admit_with(value, binder, like)?;
            admit_with(first, binder, like)?;
            admit_with(last, binder, like)
        }
        Expr::DateConvert(function, value)
            if super::mutation_scalar::date_conversion_allowed(*function) =>
        {
            admit_with(value, binder, like)
        }
        Expr::DatePart(interval, date, options) => {
            admit_with(interval, binder, like)?;
            admit_with(date, binder, like)?;

            for value in options.iter().flatten() {
                admit_with(value, binder, like)?;
            }

            Ok(())
        }
        Expr::DateDiff(interval, first, second, options) => {
            admit_with(interval, binder, like)?;
            admit_with(first, binder, like)?;
            admit_with(second, binder, like)?;

            for value in options.iter().flatten() {
                admit_with(value, binder, like)?;
            }

            Ok(())
        }
        Expr::InList(value, values, _) => {
            admit_with(value, binder, like)?;
            for value in values {
                admit_with(value, binder, like)?;
            }
            Ok(())
        }
        Expr::Nz(value, replacement) => {
            admit_with(value, binder, like)?;
            if let Some(value) = replacement {
                admit_with(value, binder, like)?;
            }
            Ok(())
        }
        // Binder::mutation closes callable admission before this structural walk.
        Expr::TextCall(_, values)
        | Expr::MathCall(_, values)
        | Expr::ConstructorCall(_, values) => {
            for value in values {
                admit_with(value, binder, like)?;
            }
            Ok(())
        }
        Expr::Exists(read) => admit_read(read, binder),
        Expr::InQuery(value, read, _) => {
            admit_with(value, binder, like)?;
            admit_read(read, binder)
        }
        _ => Err(QueryError::Unsupported),
    }
}

fn admit_read(read: &ReadPlan, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    binder.work()?;
    let ReadNode::Select(select) = &read.node else {
        return Err(QueryError::Unsupported);
    };
    if read.distinct
        || read.top.is_some()
        || !read.sort.is_empty()
        || select.aggregate
        || !select.group.is_empty()
        || select.having.is_some()
        || !select.record_keys.is_empty()
        || select.parent.is_some()
        || !read.recordset_schemas.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    let RelationNode::Table {
        namespace,
        multivalues,
        members,
        recordsets,
        ..
    } = &select.source.node
    else {
        return Err(QueryError::Unsupported);
    };
    if *namespace != crate::external_sources::SourceNamespace::Root
        || !multivalues.is_empty()
        || !members.is_empty()
        || !recordsets.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    for value in &select.projection {
        admit_with(value, binder, false)?;
    }
    if let Some(value) = &select.filter {
        admit_with(value, binder, true)?;
    }
    Ok(())
}
